use indicatif::ProgressBar;

pub trait ProgressBarExt {
    fn new_frame_progress_bar(frame_num: u64) -> ProgressBar;
}

impl ProgressBarExt for ProgressBar {
    fn new_frame_progress_bar(frame_num: u64) -> ProgressBar {
        let progress_bar = ProgressBar::new(frame_num);
        progress_bar.set_style(
            indicatif::ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}").unwrap(),
        );
        progress_bar
    }
}
