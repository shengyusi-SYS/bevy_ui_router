use crate::transaction::FrameApplyRequest;

pub fn frame_apply_request_with_next_generation(request: &FrameApplyRequest) -> FrameApplyRequest {
    request.clone_with_next_generation()
}
