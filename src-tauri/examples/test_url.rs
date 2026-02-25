use reqwest;

#[tokio::main]
async fn main() {
    let url_str = "https://login.wx.qq.com/jslogin?appid=wx_webfilehelper&redirect_uri=https%253A%252F%252Ffilehelper.weixin.qq.com%252Fcgi-bin%252Fmmwebwx-bin%252Fwebwxnewloginpage&fun=new&lang=zh_CN&_=1771996500000";
    
    println!("Input URL: {}", url_str);
    
    // Check how reqwest::Url parses it
    let parsed = reqwest::Url::parse(url_str).unwrap();
    println!("Parsed URL: {}", parsed.as_str());
    println!("Contains %253A: {}", parsed.as_str().contains("%253A"));
    println!("Contains %3A (single): {}", parsed.as_str().contains("%3A") && !parsed.as_str().contains("%253A"));
    
    // Actually send the request
    let client = reqwest::Client::builder()
        .default_headers({
            let mut h = reqwest::header::HeaderMap::new();
            h.insert("referer", "https://filehelper.weixin.qq.com/".parse().unwrap());
            h
        })
        .build()
        .unwrap();
    
    let resp = client.get(url_str).send().await.unwrap();
    println!("\nFinal URL: {}", resp.url().as_str());
    println!("Response: {}", &resp.text().await.unwrap()[..100]);
}
