
#[derive(Debug, Copy, Clone)]
pub enum CreateAccountError {
  EmailIsTaken,
  UsernameIsTaken,
  DatabaseError,
  OtherError,
}
