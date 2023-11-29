use crate::env::Var;

// custom Error and Result types to unify errors from all sources
pub type FestiveResult<T> = Result<T, FestiveError>;

#[derive(Debug)]
pub enum FestiveError
{
    Var(Var),
    Init,
    Conv,
    File,
    Http,
    Parse
}

impl std::fmt::Display for FestiveError
{
    fn fmt(&self, f : &mut std::fmt::Formatter) -> std::fmt::Result
    {
        match self
        {
            FestiveError::Var(v) => write!(f, "required environment variable {} is unset", v.key()),
            FestiveError::Init   => write!(f, "initialisation error"),
            FestiveError::Conv   => write!(f, "conversion error"),
            FestiveError::File   => write!(f, "filesystem error"),
            FestiveError::Http   => write!(f, "HTTP error"),
            FestiveError::Parse  => write!(f, "parse error")
        }
    }
}
