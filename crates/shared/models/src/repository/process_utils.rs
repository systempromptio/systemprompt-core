pub fn filter_running_services<T, F, P>(services: Vec<T>, get_pid: F, is_running: P) -> Vec<T>
where
    F: Fn(&T) -> Option<i32>,
    P: Fn(u32) -> bool,
{
    services
        .into_iter()
        .filter(|s| {
            get_pid(s)
                .and_then(|pid| u32::try_from(pid).ok())
                .is_some_and(&is_running)
        })
        .collect()
}
