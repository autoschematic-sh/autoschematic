use std::{io::Write, time::Duration};

use colored::Colorize;
use tokio::time::interval;

#[derive(Debug, Clone)]
pub struct SpinnerData {
    pub frames: &'static [&'static str],
    pub interval: u64,
}
const SPINNER: SpinnerData = SpinnerData {
    frames: &[
        //"⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"],
        //"◴", "◵", "◶", "◷",
        // "𝌆", "𝌇", "𝌈", "𝌉", "𝌊", "𝌋", "𝌌", "𝌍", "𝌎", "𝌏", "𝌐", "𝌑", "𝌒", "𝌓", "𝌔", "𝌕", "𝌖", "𝌗", "𝌘", "𝌙", "𝌚", "𝌛", "𝌜", "𝌝",
        // "𝌞", "𝌟", "𝌠", "𝌡", "𝌢", "𝌣", "𝌤", "𝌥", "𝌦", "𝌧", "𝌨", "𝌩", "𝌪", "𝌫", "𝌬", "𝌭", "𝌮", "𝌯", "𝌰", "𝌱", "𝌲", "𝌳", "𝌴", "𝌵",
        // "𝌶", "𝌷", "𝌸", "𝌹", "𝌺", "𝌻", "𝌼", "𝌽", "𝌾", "𝌿", "𝍀", "𝍁", "𝍂", "𝍃", "𝍄", "𝍅", "𝍆", "𝍇", "𝍈", "𝍉", "𝍊", "𝍋", "𝍌", "𝍍",
        // "𝍎", "𝍏", "𝍐", "𝍑", "𝍒", "𝍓", "𝍔", "𝍕", "𝍖",
        "∴", "⋰", "∵", "⋱",
    ],
    interval: 40,
};

pub async fn show_spinner() -> tokio::sync::oneshot::Sender<()> {
    let frames = SPINNER.frames;
    let mut interval = interval(Duration::from_millis(SPINNER.interval));

    let (send, mut recv) = tokio::sync::oneshot::channel::<()>();

    if crate::output::plain() {
        tokio::spawn(async move {
            let _ = recv.await;
        });
        return send;
    }

    tokio::spawn(async move {
        let mut frame_i: usize = 0;

        loop {
            {
                let lock = std::io::stdout().lock();
                print!("{}\r", frames[frame_i % frames.len()].on_black().bold());
                std::io::stdout().flush().unwrap();
                drop(lock);
            }
            frame_i += 1;
            tokio::select! {
                _ = interval.tick() => {
                    // print!("\r");
                    let _lock =std::io::stdout().lock();
                    print!(" \r");
                    std::io::stdout().flush().unwrap();
                }
                _ = &mut recv => {
                    let _lock =std::io::stdout().lock();
                    print!(" \r");
                    std::io::stdout().flush().unwrap();
                    break;
                }
            }
        }
    });

    send
}
