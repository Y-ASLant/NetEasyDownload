use std::{
    fs::File,
    io::{BufWriter, Read, Write},
    path::Path,
    sync::{Arc, RwLock, mpsc::Sender},
    time::{Duration, Instant},
};

use anyhow::{Context, Result, anyhow, bail};
use reqwest::blocking::Client;
use serde_json::{Value, json};

use crate::crypto::{aes_decrypt_hex, aes_encrypt_hex};
use crate::models::{LoginData, Song, WorkerEvent};

const BASE_URL: &str = "https://interfacepc.music.163.com/";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; WOW64) AppleWebKit/537.36 (KHTML, like Gecko) Safari/537.36 Chrome/91.0.4472.164 NeteaseMusicDesktop/3.1.23.204750";
const LOGIN_BOOTSTRAP_COOKIE: &str = "os=pc; deviceId=C2BDC8FD92695BA699BDC7C956411CC93655528ADE567B471163; osver=Microsoft-Windows-11-Home-China-build-26100-64bit; channel=netease; clientSign=18:3D:2D:2B:3E:C2@@@354344465F423830355F353041305F314646362E@@@@@@b1273282e030b475e4eade199c0df7f1f52a630fe8ca726441c26e62d232e255; mode=83NN; appver=3.1.23.204750; NMTID=00OVOVh-tLKvM3DZUSCmyRL3C0S14MAAAGXi4UwBA; WEVNSM=1.0.0; MUSIC_U=00EE3A756CF88ACD55E7D04EA21DB9EF20CF07F7031A1120ED289F1BAABEDF568963BA441940E6FAE22230E0ACCED413F375BFBE5F60FFD2C352EFC74909E58F8196115A416A4274D388CD87FCD0D7F68E261DAC071AE88EF7AEAF1699E2A813FF7BEEDDC85E32FB8812E5121503524DA83DC202D2409F10DA1426A96928B377141133B9A6FE379832349ACB179D7E67FED93C532766701FB5BF50EF1A6386DB9BEF4716853BF5930557A0D8EB2DAA58BD195FC515DB64137BA4130FE077726333CC7424AB2675BE488303EA5FFCE5F4534F39151287BF30021A53C9B2610CDE41670DAD4BD87B1FE31DCCADCD1E1BCF745B128157C768DBC95740F9299D778E88CF3CC64C1CF41CC031A0AC85D9CEAED002C1078BA40D146766EE3231F8B4B4FDD809EE1555DB1A3BF0C36855B6447D2F3C5DC85D5CB994925A62DDCD652A390C5B85BA22204555D213E84A42F993C15984DC0C0B13529ED6FFD0D65204F96BDAB4F815D20AD57E2FADC04FDF1E3A42A739914029C1B71A105AAF6AE450A465CA063111080109A3FAF331D8CE8DE3DA659F31B343CAE14C0BA279AB0142F65C0A90089F8C08848BE772483410041FE88746ACAEEC28FFF2EB18256911DC2880D1A1EE3F87305C44FDB465F63AFAEF209D343B345B0E3E13ECF4B11EF7F5A91E9DCBE3EE30C91E55BB70A0880B735AD64E; __csrf=4a9a0ac890edc56f0710fe5e28020d62; __remember_me=true";
const DOWNLOAD_BUFFER_SIZE: usize = 64 * 1024;
const DOWNLOAD_PROGRESS_BYTES_STEP: u64 = 256 * 1024;
const DOWNLOAD_PROGRESS_INTERVAL: Duration = Duration::from_millis(200);

#[derive(Clone)]
pub struct MusicClient {
    http: Client,
    cookie: Arc<RwLock<String>>,
}

impl MusicClient {
    pub fn new(cookie: String) -> Result<Self> {
        let http = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent(USER_AGENT)
            .build()
            .context("创建 HTTP 客户端失败")?;

        Ok(Self {
            http,
            cookie: Arc::new(RwLock::new(cookie)),
        })
    }

    pub fn set_cookie(&self, cookie: String) {
        if let Ok(mut guard) = self.cookie.write() {
            *guard = cookie;
        }
    }

    pub fn search_music(&self, keyword: &str, offset: usize, limit: usize) -> Result<Vec<Song>> {
        let body_json = json!({
            "keyword": keyword,
            "scene": "NORMAL",
            "limit": limit.to_string(),
            "offset": offset.to_string(),
            "needCorrect": "true",
            "e_r": true,
            "checkToken": "",
            "header": ""
        })
        .to_string();

        let plain = self.post_eapi(
            "/api/search/song/list/page",
            "eapi/search/song/list/page",
            &body_json,
        )?;

        parse_search_result(&plain)
    }

    pub fn send_captcha(&self, phone: &str) -> Result<()> {
        let body_json = json!({
            "cellphone": phone,
            "ctcode": "86",
            "secrete": "music_middleuser_pclogin",
            "e_r": true,
            "header": ""
        })
        .to_string();

        let plain = self.post_eapi_with_cookie(
            "/api/sms/captcha/sent",
            "eapi/sms/captcha/sent",
            &body_json,
            Some(LOGIN_BOOTSTRAP_COOKIE),
        )?;

        let value: Value = serde_json::from_str(&plain).context("解析验证码响应失败")?;
        let code = value
            .get("code")
            .and_then(Value::as_i64)
            .unwrap_or_default();
        let data_ok = value
            .get("data")
            .map(|v| match v {
                Value::String(s) => s.eq_ignore_ascii_case("true"),
                Value::Bool(b) => *b,
                _ => false,
            })
            .unwrap_or(false);

        if code == 200 && data_ok {
            Ok(())
        } else {
            let msg = value
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("接口返回失败");
            bail!("验证码发送失败: code={code}, message={msg}");
        }
    }

    pub fn login_with_captcha(&self, phone: &str, captcha: &str) -> Result<LoginData> {
        let body_json = json!({
            "type": "1",
            "phone": phone,
            "captcha": captcha,
            "remember": "true",
            "https": "true",
            "countrycode": "86",
            "e_r": true,
            "header": ""
        })
        .to_string();

        let plain = self.post_eapi_with_cookie(
            "/api/w/login/cellphone",
            "eapi/w/login/cellphone",
            &body_json,
            Some(LOGIN_BOOTSTRAP_COOKIE),
        )?;

        let value: Value = serde_json::from_str(&plain).context("解析登录响应失败")?;
        let code = value
            .get("code")
            .and_then(Value::as_i64)
            .unwrap_or_default();
        if code != 200 {
            let message = value
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("未知错误");
            bail!("登录失败: code={code}, message={message}");
        }

        let user_id = value
            .get("account")
            .and_then(|v| v.get("id"))
            .and_then(|v| {
                v.as_str()
                    .map(str::to_string)
                    .or_else(|| v.as_i64().map(|n| n.to_string()))
                    .or_else(|| v.as_u64().map(|n| n.to_string()))
            })
            .ok_or_else(|| anyhow!("登录响应缺少 account.id"))?;

        let token = value
            .get("token")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_string();

        if token.is_empty() {
            bail!("登录响应缺少 token");
        }

        Ok(LoginData { user_id, token })
    }

    pub fn get_song_url(&self, song_id: u64, level: &str) -> Result<String> {
        let body_json = json!({
            "ids": format!("[\"{song_id}\"]"),
            "level": level,
            "immerseType": "c51",
            "encodeType": "aac",
            "trialMode": "-1",
            "e_r": true,
            "header": ""
        })
        .to_string();

        let plain = self.post_eapi(
            "/api/song/enhance/player/url/v1",
            "eapi/song/enhance/player/url/v1",
            &body_json,
        )?;

        let value: Value = serde_json::from_str(&plain).context("解析歌曲 URL 响应失败")?;
        let data0 = value
            .get("data")
            .and_then(Value::as_array)
            .and_then(|arr| arr.first())
            .ok_or_else(|| anyhow!("歌曲 URL 响应不包含 data[0]"))?;

        let url = data0
            .get("url")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_string();

        if url.is_empty() {
            bail!("没有可用下载地址（可能需要登录 cookie 或歌曲受限）");
        }

        Ok(url)
    }

    pub fn download_file_with_progress(
        &self,
        url: &str,
        output: &Path,
        tx: &Sender<WorkerEvent>,
    ) -> Result<()> {
        let mut resp = self
            .http
            .get(url)
            .send()
            .context("请求下载链接失败")?
            .error_for_status()
            .context("下载响应状态异常")?;

        let total = resp.content_length();
        let file =
            File::create(output).with_context(|| format!("创建文件失败: {}", output.display()))?;
        let mut file = BufWriter::new(file);
        let mut downloaded = 0u64;
        let mut buf = [0u8; DOWNLOAD_BUFFER_SIZE];
        let mut last_reported = 0u64;
        let mut last_report_at = Instant::now();

        loop {
            let n = resp.read(&mut buf).context("读取下载流失败")?;
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n]).context("写入文件失败")?;
            downloaded += n as u64;
            if downloaded.saturating_sub(last_reported) >= DOWNLOAD_PROGRESS_BYTES_STEP
                || last_report_at.elapsed() >= DOWNLOAD_PROGRESS_INTERVAL
            {
                let _ = tx.send(WorkerEvent::DownloadProgress { downloaded, total });
                last_reported = downloaded;
                last_report_at = Instant::now();
            }
        }

        file.flush().context("flush buffered file writes failed")?;

        if downloaded != last_reported {
            let _ = tx.send(WorkerEvent::DownloadProgress { downloaded, total });
        }

        Ok(())
    }

    fn current_cookie(&self) -> String {
        self.cookie
            .read()
            .map(|g| g.clone())
            .unwrap_or_else(|_| String::new())
    }

    fn post_eapi(&self, api_path: &str, endpoint: &str, body_json: &str) -> Result<String> {
        let cookie = self.current_cookie();
        let cookie_opt = if cookie.trim().is_empty() {
            None
        } else {
            Some(cookie)
        };

        self.post_eapi_with_cookie(api_path, endpoint, body_json, cookie_opt.as_deref())
    }

    fn post_eapi_with_cookie(
        &self,
        api_path: &str,
        endpoint: &str,
        body_json: &str,
        cookie: Option<&str>,
    ) -> Result<String> {
        let digest = format!(
            "{:x}",
            md5::compute(format!("nobody{api_path}use{body_json}md5forencrypt"))
        );
        let query = format!("{api_path}-36cd479b6b5-{body_json}-36cd479b6b5-{digest}");
        let encrypted = aes_encrypt_hex(&query)?;

        let url = format!("{BASE_URL}{endpoint}");
        let mut request = self
            .http
            .post(&url)
            .form(&[("params", encrypted)])
            .header("Accept", "*/*")
            .header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header("Referer", "https://music.163.com/")
            .header("Origin", "https://music.163.com");

        if let Some(cookie_value) = cookie
            && !cookie_value.trim().is_empty()
        {
            request = request.header("Cookie", cookie_value);
        }

        let response = request
            .send()
            .context("请求 eapi 接口失败")?
            .error_for_status()
            .context("eapi 响应状态异常")?;

        let bytes = response.bytes().context("读取 eapi 响应失败")?;
        let encrypted_hex = hex::encode(bytes);
        aes_decrypt_hex(&encrypted_hex)
    }
}

fn parse_search_result(json: &str) -> Result<Vec<Song>> {
    let value: Value = serde_json::from_str(json).context("解析搜索响应失败")?;
    let code = value
        .get("code")
        .and_then(Value::as_i64)
        .unwrap_or_default();
    if code != 200 {
        let message = value
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("未知错误");
        bail!("搜索失败: code={code}, message={message}");
    }

    let mut songs = Vec::new();
    if let Some(resources) = value
        .get("data")
        .and_then(|v| v.get("resources"))
        .and_then(Value::as_array)
    {
        for item in resources {
            let Some(song_data) = item.get("baseInfo").and_then(|v| v.get("simpleSongData")) else {
                continue;
            };

            let id = song_data
                .get("id")
                .and_then(Value::as_u64)
                .or_else(|| {
                    song_data
                        .get("id")
                        .and_then(Value::as_i64)
                        .map(|n| n as u64)
                })
                .unwrap_or(0);
            if id == 0 {
                continue;
            }

            let name = song_data
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("<未知歌曲>")
                .to_string();

            let artists = song_data
                .get("ar")
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter_map(|a| a.get("name").and_then(Value::as_str).map(str::to_string))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            songs.push(Song { id, name, artists });
        }
    }

    Ok(songs)
}
