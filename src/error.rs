// custom Error and Result types to unify errors from all sources
pub type FestiveResult<T> = Result<T, FestiveError>;

#[derive(Debug)]
pub enum FestiveError
{
    Var(EnvVar),
    Arg,
    Init,
    Conv,
    File,
    Http,
    Parse
}

// environment variables used by the program
#[derive(Debug)]
pub enum EnvVar { Leaderboard, Session, Notify, Status }

impl EnvVar
{
    fn key(&self) -> &'static str
    {
        match self
        {
            EnvVar::Leaderboard => "FESTIVE_BOT_LEADERBOARD",
            EnvVar::Session     => "FESTIVE_BOT_SESSION",
            EnvVar::Notify      => "FESTIVE_BOT_NOTIFY",
            EnvVar::Status      => "FESTIVE_BOT_STATUS"
        }
    }

    pub fn get(self) -> FestiveResult<String>
    {
        std::env::var(self.key()).map_err(|_| FestiveError::Var(self))
    }
}
