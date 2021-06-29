#[macro_use]
extern crate jsonapi;
#[macro_use]
extern crate serde;

use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum Error {
    #[error("Empty Response")]
    Empty,
    #[error("Wrong Response from api")]
    Api(jsonapi::api::DocumentError),
    #[error(transparent)]
    Parse(#[from] jsonapi::errors::Error),
    #[error(transparent)]
    Request(#[from] reqwest::Error),
    #[error(transparent)]
    Url(#[from] url::ParseError),
}

pub type Result<T> = std::result::Result<T, Error>;

pub mod models {
    use chrono::{DateTime, Utc};
    use jsonapi::api::*;
    use jsonapi::model::*;

    use crate::{Error, Result};

    pub trait ParseJsonApi: JsonApiModel {
        fn from_document(document: JsonApiDocument) -> Result<Self> {
            match document {
                JsonApiDocument::Data(doc) => {
                    Self::from_jsonapi_document(&doc).map_err(|err| Error::Parse(err))
                }
                JsonApiDocument::Error(err) => Err(Error::Api(err).into()),
            }
        }

        fn collection_from_document(document: JsonApiDocument) -> Result<Vec<Self>> {
            match document {
                JsonApiDocument::Data(doc) => match doc.data {
                    Some(data) => match data {
                        PrimaryData::None => Ok(Vec::new()),
                        PrimaryData::Single(resource) => {
                            Self::from_jsonapi_resource(&resource, &None)
                                .map(|a| vec![a])
                                .map_err(|err| Error::Parse(err).into())
                        }
                        PrimaryData::Multiple(resources) => resources
                            .iter()
                            .map(|res| {
                                Self::from_jsonapi_resource(&res, &None)
                                    .map_err(|err| Error::Parse(err).into())
                            })
                            .collect(),
                    },
                    None => Err(Error::Empty.into()),
                },
                JsonApiDocument::Error(err) => Err(Error::Api(err).into()),
            }
        }
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
    pub struct ImageSet {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub tiny: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub small: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub medium: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub large: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub original: Option<String>,
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
    pub struct Anime {
        pub id: String,
        #[serde(rename = "createdAt")]
        pub created_at: DateTime<Utc>,
        #[serde(rename = "updatedAt")]
        pub updated_at: DateTime<Utc>,
        pub slug: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub synopsis: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub description: Option<String>,
        pub titles: HashMap<String, String>,
        #[serde(rename = "canonicalTitle")]
        pub canonical_title: String,
        #[serde(rename = "abbreviatedTitles")]
        pub abbreviated_titles: Vec<String>,
        #[serde(rename = "averageRating")]
        #[serde(skip_serializing_if = "Option::is_none")]
        pub rating: Option<String>,
        #[serde(rename = "showType")]
        pub show_type: String,
        #[serde(rename = "subType")]
        #[serde(skip_serializing_if = "Option::is_none")]
        pub sub_type: Option<String>,
        #[serde(rename = "posterImage")]
        #[serde(skip_serializing_if = "Option::is_none")]
        pub poster_image: Option<ImageSet>,
        #[serde(rename = "coverImage")]
        #[serde(skip_serializing_if = "Option::is_none")]
        pub cover_image: Option<ImageSet>,
        #[serde(rename = "episodeCount")]
        #[serde(skip_serializing_if = "Option::is_none")]
        pub episode_count: Option<i32>,
    }

    jsonapi_model!(Anime; "anime");
    impl ParseJsonApi for Anime {}
}

pub mod api {
    use jsonapi::api::*;
    use reqwest;
    use url::Url;

    use crate::{Error, Result};

    use super::models::ParseJsonApi;

    const JSON_API_TYPE: &str = "application/vnd.api+json";
    const ACCEPT_HEADER: &str = "Accept";
    const CONTENT_TYPE_HEADER: &str = "Content-Type";

    fn get_url_builder(url: Url) -> reqwest::RequestBuilder {
        reqwest::Client::new()
            .get(url)
            .header(ACCEPT_HEADER, JSON_API_TYPE)
            .header(CONTENT_TYPE_HEADER, JSON_API_TYPE)
    }

    async fn get_document(url: Url) -> Result<JsonApiDocument> {
        let document: JsonApiDocument = get_url_builder(url).send().await?.json().await?;
        return Ok(document);
    }

    pub(self) async fn get_resource<T: ParseJsonApi>(url: Url) -> Result<T> {
        let doc = get_document(url).await?;
        T::from_document(doc)
    }

    pub(self) async fn get_resources<T: ParseJsonApi>(url: Url) -> Result<Vec<T>> {
        let doc = get_document(url).await?;
        T::collection_from_document(doc)
    }

    pub mod anime {
        use jsonapi::model::HashMap;
        use jsonapi::query::{PageParams, Query};
        use url::Url;

        use crate::{api, models, Error, Result};

        pub async fn get_resource(id: u64) -> Result<models::Anime> {
            let url_string = format!("https://kitsu.io/api/edge/anime/{}", id);
            let url = Url::parse(&url_string)?;
            let anime = api::get_resource::<models::Anime>(url).await?;
            return Ok(anime);
        }

        pub async fn get_collection<S: AsRef<str>>(title: S) -> Result<Vec<models::Anime>> {
            let url_string = "https://kitsu.io/api/edge/anime";
            let mut url = Url::parse(url_string)?;
            let mut map = HashMap::new();
            map.insert("text".to_string(), vec![title.as_ref().into()]);
            let query = Query {
                sort: None,
                _type: "anime".to_string(),
                page: Some(PageParams {
                    number: 0,
                    size: 50,
                }),
                filter: Some(map),
                fields: None,
                include: None,
            }
            .to_params();
            url.set_query(Some(&query));
            let anime = api::get_resources::<models::Anime>(url).await?;
            return Ok(anime);
        }
    }
}
