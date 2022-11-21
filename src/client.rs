use super::signals::Signal;
use super::consts::UNIX_PIPE_FILE_NAME;
use log::debug;

/// Writes signal invoked in the client to the pipe
pub fn invoke(signal: Signal) {
    let pipe_path = UNIX_PIPE_FILE_NAME.as_path();
    let config = bincode::config::standard();

    // Write signal to the pipe
    let mut writer =
    unix_named_pipe::open_write(pipe_path).expect("could not open test pipe for writing");

    let len = bincode::encode_into_std_write(&signal, &mut writer, config).unwrap();

    debug!("Sended {signal:?} to the daemon with len {len}");
    client_info_wallpaper_change_invoked(&signal)
}

/// Wrapper to print nicely
fn client_info_wallpaper_change_invoked(signal: &Signal) {
    let str_to_print = match signal {
        Signal::Next => "Changing to the next wallpaper",
        Signal::Prev => "Changing to the previous wallpaper",
    };

    println!("{str_to_print}");
}
