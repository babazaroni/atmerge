// git tag v0.1.0  this does not get pushed to remote at regular git push
// git push origin v0.1.0  only pushes tag
// git push

// asset name x86_64-unknown-linux-gnu
// asset x86_64-pc-windows-msvc

// go back to previous version:
// git checkout <commit-id>

// list commit ids
// git log -n 50 --oneline^C

// git checkout <commit-id>
// git checkout main  // to go back to main branch

use std::sync::mpsc::Sender;

use self_update::update::Release;

include!("macros.rs");

use self_update;

pub fn get_releases() -> Result<Vec<Release>, Box<dyn ::std::error::Error>> {
    let mut rel_builder = self_update::backends::github::ReleaseList::configure();

    #[cfg(feature = "signatures")]
    rel_builder.repo_owner("Kijewski");
    #[cfg(not(feature = "signatures"))]
    rel_builder.repo_owner("babazaroni");

    let releases = rel_builder.repo_name("atmerge").build()?.fetch()?;

    println!("get releases returning");
    Ok(releases)
}

pub fn get_newer_release() -> Result<Option<Release>, Box<dyn ::std::error::Error>> {
    let releases = get_releases()?;
    let current_version = cargo_crate_version!();
    let current_version = semver::Version::parse(current_version)?;

    let newer_release = releases
        .iter()
        .filter(|r| {
            semver::Version::parse(&r.version).map(|v| v > current_version).unwrap_or(false)
        })
        .max_by(|a, b| {
            semver::Version::parse(&a.version)
                .unwrap()
                .cmp(&semver::Version::parse(&b.version).unwrap())
        });
        println!("get newer release returning");
    Ok(newer_release.cloned())
}

pub fn atmerge_self_update(target_version:String) -> Result<(), Box<dyn ::std::error::Error>> {

    get_releases()?;

    let mut status_builder = self_update::backends::github::Update::configure();

    #[cfg(feature = "signatures")]
    status_builder
        .repo_owner("Kijewski")
        .verifying_keys([*include_bytes!("github-public.key")]);
    #[cfg(not(feature = "signatures"))]
    status_builder.repo_owner("babazaroni");

    let status = status_builder
        .repo_name("atmerge")
        .bin_name("atmerge")
        .show_download_progress(true)
        .target_version_tag(&target_version.as_str())
        //.show_output(false)
        .no_confirm(true)
        //
        // For private repos, you will need to provide a GitHub auth token
        // **Make sure not to bake the token into your app**; it is recommended
        // you obtain it via another mechanism, such as environment variables
        // or prompting the user for input
        //.auth_token(env!("DOWNLOAD_AUTH_TOKEN"))
        .current_version(cargo_crate_version!())
        .build()?
        .update()?;
    println!("Update status: `{}`!", status.version());
    println!("atmerge_self_update returning");
    Ok(())
}


pub fn start_update_monitor(ctx: egui::Context,tx_update:Sender<bool>) {


    tokio::spawn(async move {

        loop {
            let _ = tx_update.send(true);
            ctx.request_repaint(); 
            std::thread::sleep(std::time::Duration::from_secs(15))        
        }

});
}
