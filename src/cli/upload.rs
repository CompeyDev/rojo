use reqwest::header::{ACCEPT, CONTENT_TYPE, COOKIE, USER_AGENT};
use snafu::{ResultExt, Snafu};

use crate::{
    auth_cookie::get_auth_cookie,
    cli::UploadCommand,
    common_setup,
    vfs::{RealFetcher, Vfs, WatchMode},
};

#[derive(Debug, Snafu)]
pub struct UploadError(Error);

#[derive(Debug, Snafu)]
enum Error {
    #[snafu(display(
        "Rojo could not find your Roblox auth cookie. Please pass one via --cookie.",
    ))]
    NeedAuthCookie,

    #[snafu(display("XML model file encode error: {}", source))]
    XmlModel { source: rbx_xml::EncodeError },

    #[snafu(display("HTTP error: {}", source))]
    Http { source: reqwest::Error },

    #[snafu(display("Roblox API error: {}", body))]
    RobloxApi { body: String },
}

pub fn upload(options: UploadCommand) -> Result<(), UploadError> {
    Ok(upload_inner(options)?)
}

fn upload_inner(options: UploadCommand) -> Result<(), Error> {
    let cookie = options
        .cookie
        .or_else(get_auth_cookie)
        .ok_or(Error::NeedAuthCookie)?;

    log::trace!("Constructing in-memory filesystem");
    let vfs = Vfs::new(RealFetcher::new(WatchMode::Disabled));

    let (_maybe_project, tree) = common_setup::start(&options.project, &vfs);
    let root_id = tree.get_root_id();

    let mut buffer = Vec::new();

    log::trace!("Encoding XML model");
    let config = rbx_xml::EncodeOptions::new()
        .property_behavior(rbx_xml::EncodePropertyBehavior::WriteUnknown);
    rbx_xml::to_writer(&mut buffer, tree.inner(), &[root_id], config).context(XmlModel)?;

    let url = format!(
        "https://data.roblox.com/Data/Upload.ashx?assetid={}",
        options.asset_id
    );

    log::trace!("POSTing to {}", url);
    let client = reqwest::Client::new();
    let mut response = client
        .post(&url)
        .header(COOKIE, format!(".ROBLOSECURITY={}", &cookie))
        .header(USER_AGENT, "Roblox/WinInet")
        .header("Requester", "Client")
        .header(CONTENT_TYPE, "application/xml")
        .header(ACCEPT, "application/json")
        .body(buffer)
        .send()
        .context(Http)?;

    if !response.status().is_success() {
        return Err(Error::RobloxApi {
            body: response.text().context(Http)?,
        });
    }

    Ok(())
}
