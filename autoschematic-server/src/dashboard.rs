pub mod routes;
pub mod api_util;

use std::sync::Mutex;

use once_cell::sync::OnceCell;
use tera::Tera;

lazy_static::lazy_static! {
    pub(crate) static ref HOT_TEMPLATES: Mutex<Tera> = Mutex::new(Tera::new("dashboard/**/*.html").unwrap());
}

pub(crate) static TEMPLATES: OnceCell<Tera> = OnceCell::new();