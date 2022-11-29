// custom Error and Result types to unify errors from all sources
pub type FestiveResult<T> = Result<T, FestiveError>;

#[derive(Debug)]
pub enum FestiveError
{
    EnvVar(&'static str),
    Init,
    Conv,
    File,
    Http,
    Parse
}
