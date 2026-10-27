use super::*;

#[test]
fn conflict() {
    for prefix in ["", "adapt: "] {
        let error = ApiError::from_service(format!(
            "{prefix}job capability conflicts with its accepted request"
        ));
        assert_eq!(error.status(), StatusCode::CONFLICT);
        assert_eq!(
            error.message(),
            "job capability conflicts with its accepted request"
        );
    }
}
