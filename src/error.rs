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
