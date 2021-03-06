use actix_web::{HttpResponse, State};
use db::Connection;
use errors::BigNeonError;
use server::AppState;
use utils::gen_sitemap;

pub fn index((connection, state): (Connection, State<AppState>)) -> Result<HttpResponse, BigNeonError> {
    let conn = connection.get();

    let sitemap_xml = gen_sitemap::create_sitemap_conn(conn, &state.config.front_end_url)?;

    Ok(HttpResponse::Ok().content_type("text/xml").body(sitemap_xml).into())
}
