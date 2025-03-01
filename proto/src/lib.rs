use prost_types::Timestamp;
use std::num::{ParseIntError, TryFromIntError};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    ParseInt(#[from] ParseIntError),
    #[error(transparent)]
    TryFromIntError(#[from] TryFromIntError),
    #[error("Missing required field: {0}")]
    MissingField(&'static str),
    #[error("Encounter invalid timestamp")]
    InvalidTimeStamp(Timestamp),
}

pub mod api {
    pub mod v2 {
        #![allow(clippy::large_enum_variant)]
        #![allow(clippy::derive_partial_eq_without_eq)]

        use crate::Error;
        use chrono::{DateTime, Utc};
        use domain::DownloadVariant;
        use prost_types::Timestamp;

        tonic::include_proto!("api.v2");

        impl TryFrom<DownloadCollection> for domain::DownloadCollection {
            type Error = Error;

            fn try_from(value: DownloadCollection) -> Result<Self, Self::Error> {
                let download_variant = value.variant.ok_or(Error::MissingField("variant"))?;
                let created_at_timestamp =
                    value.created_at.ok_or(Error::MissingField("created_at"))?;
                let updated_at_timestamp =
                    value.updated_at.ok_or(Error::MissingField("updated_at"))?;

                Ok(domain::DownloadCollection {
                    title: value.title,
                    variant: download_variant.into(),
                    created_at: from_timestamp(created_at_timestamp)?,
                    updated_at: from_timestamp(updated_at_timestamp)?,
                    downloads: value
                        .downloads
                        .into_iter()
                        .map(domain::Download::try_from)
                        .collect::<Result<Vec<_>, _>>()?,
                })
            }
        }

        impl From<download_collection::Variant> for DownloadVariant {
            fn from(value: download_collection::Variant) -> Self {
                match value {
                    download_collection::Variant::Batch(range) => {
                        DownloadVariant::Batch(range.start..=range.end)
                    }
                    download_collection::Variant::Episode(ep) => {
                        DownloadVariant::Episode(ep.into())
                    }
                    download_collection::Variant::Movie(_) => DownloadVariant::Movie,
                }
            }
        }

        impl From<Episode> for domain::Episode {
            fn from(val: Episode) -> Self {
                Self {
                    number: val.number,
                    decimal: Some(val.decimal).filter(|&d| d != 0),
                    version: Some(val.version).filter(|&d| d != 0),
                    extra: Some(val.extra).filter(|s| !s.is_empty()),
                }
            }
        }

        impl TryFrom<Download> for domain::Download {
            type Error = Error;

            fn try_from(value: Download) -> Result<Self, Self::Error> {
                let published_date_timestamp = value
                    .published_date
                    .ok_or(Error::MissingField("published_date"))?;
                Ok(domain::Download {
                    published_date: from_timestamp(published_date_timestamp)?,
                    resolution: u16::try_from(value.resolution)?,
                    comments: value.comments,
                    torrent: value.torrent,
                    file_name: value.file_name,
                })
            }
        }

        #[allow(clippy::cast_sign_loss)]
        fn from_timestamp(timestamp: Timestamp) -> Result<DateTime<Utc>, Error> {
            DateTime::from_timestamp(timestamp.seconds, timestamp.nanos as u32)
                .ok_or(Error::InvalidTimeStamp(timestamp))
        }
    }
}
