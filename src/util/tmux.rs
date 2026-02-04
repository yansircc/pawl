use anyhow::Result;

pub fn create_session(_name: &str) -> Result<()> {
    todo!("tmux::create_session not implemented")
}

pub fn create_window(_session: &str, _window: &str) -> Result<()> {
    todo!("tmux::create_window not implemented")
}

pub fn send_keys(_session: &str, _window: &str, _keys: &str) -> Result<()> {
    todo!("tmux::send_keys not implemented")
}

pub fn attach(_session: &str) -> Result<()> {
    todo!("tmux::attach not implemented")
}

pub fn kill_session(_name: &str) -> Result<()> {
    todo!("tmux::kill_session not implemented")
}
