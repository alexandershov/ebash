use anyhow::Result;
use prost_build::Config;

fn main() -> Result<()> {
    // use protoc_bin_vendored to make proto compilation hermetic, because prost requires `protoc`
    let protoc_path = protoc_bin_vendored::protoc_bin_path()?;

    let mut prost_build = Config::new();
    prost_build.protoc_executable(protoc_path);
    prost_build.compile_protos(&["generated/config.proto"], &[] as &[String])?;
    Ok(())
}
