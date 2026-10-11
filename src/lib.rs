// make aa new crate for aion_com
// pub trait LogExt<T> {
//     fn log_ok(self) -> Option<T>;
// }

// impl<T, E: std::fmt::Display> LogExt<T> for Result<T, E> {
//     fn log_ok(self) -> Option<T> {
//         match self {
//             Ok(v) => Some(v),
//             Err(e) => {
//                 event!(Level::TRACE, "Dropping Error: {}", e);
//                 None
//             }
//         }
//     }
// }