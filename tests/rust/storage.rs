//! Completion-based local storage through the existing reader and HTTP interfaces.

use super::common::new_test_state;
use aphanite::storage::AssetStorage;

use futures_lite::io::Cursor;
use ntex::http::{StatusCode, header};
use ntex::web::{self, test};

#[ntex::test]
async fn local_storage_roundtrip_ranges_and_reference_counts() {
    async {
        let tmp = tempfile::tempdir().unwrap();
        let state = new_test_state(tmp.path()).await.unwrap();
        // Cross both the 8 KiB upload and 64 KiB download buffer boundaries.
        let content: Vec<u8> = (0..160_003).map(|i| (i % 251) as u8).collect();
        let file = state
            .assets
            .create_file(Cursor::new(&content))
            .await
            .unwrap();
        assert_eq!(file.hash, blake3::hash(&content).to_hex().as_str());
        let duplicate = state
            .assets
            .create_file(Cursor::new(&content))
            .await
            .unwrap();
        assert_eq!(file.id, duplicate.id);
        assert_eq!(
            std::fs::read_dir(tmp.path().join("tmp")).unwrap().count(),
            0
        );

        let app = test::init_service_st(
            web::AppState::new(state.clone()),
            web::App::new().service(web::scope("/assets").configure(AssetStorage::configure)),
        )
        .await;
        let uri = format!("/assets/{}", file.hash);
        let response =
            test::call_service(&app, test::TestRequest::with_uri(&uri).to_request()).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(test::read_body(response).await.as_ref(), content.as_slice());

        for (range, start, end) in [
            ("bytes=8191-139999", 8191, 139999),
            ("bytes=-7", content.len() - 7, content.len() - 1),
            ("bytes=65535-", 65535, content.len() - 1),
            ("bytes=5-5", 5, 5),
        ] {
            let response = test::call_service(
                &app,
                test::TestRequest::with_uri(&uri)
                    .header(header::RANGE, range)
                    .to_request(),
            )
            .await;
            assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
            let expected_range = format!("bytes {start}-{end}/{}", content.len());
            assert_eq!(
                response
                    .headers()
                    .get(header::CONTENT_RANGE)
                    .unwrap()
                    .to_str()
                    .unwrap(),
                expected_range
            );
            assert_eq!(
                test::read_body(response).await.as_ref(),
                &content[start..=end]
            );
        }

        assert_eq!(
            state.assets.delete_file(file.id).await,
            Some(file.hash.clone())
        );
        let response =
            test::call_service(&app, test::TestRequest::with_uri(&uri).to_request()).await;
        assert_eq!(test::read_body(response).await.as_ref(), content.as_slice());
        assert_eq!(state.assets.delete_file(file.id).await, Some(file.hash));
        let response =
            test::call_service(&app, test::TestRequest::with_uri(&uri).to_request()).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let empty = state
            .assets
            .create_file(Cursor::new(Vec::<u8>::new()))
            .await
            .unwrap();
        let empty_uri = format!("/assets/{}", empty.id);
        let response =
            test::call_service(&app, test::TestRequest::with_uri(&empty_uri).to_request()).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert!(test::read_body(response).await.is_empty());
    }
    .await;
}
