use std::fs;

fn main() {
    let paths = fs::read_dir("./proto").unwrap();

    protoc::ProtocLangOut::new()
        .lang("rust")
        .out_dir("src")
        .inputs(paths.map(|x| x.unwrap().path().display().to_string()).collect::<Vec<String>>())
        .run()
        .unwrap();
}
