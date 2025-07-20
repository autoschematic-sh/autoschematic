use crate::spinner::utils::spinner_data::SpinnerData;
use lazy_static::lazy_static;
lazy_static! {
    pub static ref SPINNER_FRAMES: SpinnerData = SpinnerData {
        frames: vec!["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"],
        interval: 80
    };
}
