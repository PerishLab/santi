use super::read;

#[test]
fn deadline() {
    assert_eq!(read("").execution.budget().unwrap().timeout, 60);
    assert_eq!(
        read("[execution]\ntimeout = 7")
            .execution
            .budget()
            .unwrap()
            .timeout,
        7
    );
    for seconds in [0, 3601] {
        let config = read(&format!("[execution]\ntimeout = {seconds}"));
        assert!(config.execution.budget().unwrap_err().contains("timeout"));
    }
}
