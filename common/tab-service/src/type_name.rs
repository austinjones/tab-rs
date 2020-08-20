use regex::Regex;

pub fn type_name<T>() -> String {
    let name = std::any::type_name::<T>();

    let regex = Regex::new("[a-z][A-Za-z0-9_]+::").expect("Regex compiles");
    regex.replace_all(name, "").to_string()
}
