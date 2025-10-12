use embed_resource::NONE;

fn main() {
    let re = embed_resource::compile("resources.rc", NONE);
    re.manifest_optional().unwrap();
}
