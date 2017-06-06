error_chain! {
    types {
        Error, ErrorKind, ResultExt;
    }

    links {
    }

    foreign_links {
        Io(::std::io::Error);
        Reqwest(::reqwest::Error);
        SerdeJson(::serde_json::Error);
    }

    errors {
    }
}