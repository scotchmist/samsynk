use reqwest;
use test_context::test_context;
use crate::setup::TestContext;


#[test_context(TestContext)]
#[tokio::test]
async fn check_api_healthcheck(tctx: &TestContext) -> Result<(), reqwest::Error> {
    let ret = tctx.http_get("api/healthcheck").await?;
    assert_eq!(ret.status(), reqwest::StatusCode::OK);
    Ok(())
}

#[test_context(TestContext)]
#[tokio::test]
async fn check_api_404(tctx: &TestContext) -> Result<(), reqwest::Error> {
    let ret = tctx.http_get("api/foo").await?;
    assert_eq!(ret.status(), reqwest::StatusCode::NOT_FOUND);
    Ok(())
}