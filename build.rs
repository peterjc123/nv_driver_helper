use std::env;

fn main() {
    let profile = env::var("PROFILE").unwrap();

    if profile == "release" {
        let mut res = winres::WindowsResource::new();
        res.set_icon("res\\icon.ico")
           .set("InternalName", "nv_driver_helper.exe")
           // manually set version 1.0.0.0
           .set_version_info(winres::VersionInfo::PRODUCTVERSION, 0x0001000000000000);
        res.compile().expect("Resource compile error");

        println!("cargo:rustc-link-arg=/LTCG");
    }
}