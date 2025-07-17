
#[derive(Debug, PartialEq, Eq)]
pub enum SystemStatus {
    Init,
    Executing,
    Pending,
    Executed,
}