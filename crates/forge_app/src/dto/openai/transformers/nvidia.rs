use forge_domain::Transformer;

use crate::dto::openai::{ChatTemplateKwargs, Request};

/// Transformer that applies NVIDIA NIM-specific parameter adjustments
///
/// NVIDIA NIM hosts models that may require special parameters:
/// - Models with thinking support (e.g. z-ai/glm-5.1) need
///   `chat_template_kwargs` with `enable_thinking: true` and
///   `clear_thinking: false`
/// - Minimax models on NVIDIA (e.g. minimaxai/minimax-m2.7) need specific
///   temperature and top_p values
pub struct SetNvidiaParams;

impl Transformer for SetNvidiaParams {
    type Value = Request;

    fn transform(&mut self, mut request: Self::Value) -> Self::Value {
        let model_id = request
            .model
            .as_ref()
            .map(|m| m.as_str().to_lowercase())
            .unwrap_or_default();

        // Models that support thinking via chat_template_kwargs on NVIDIA NIM
        if model_id.contains("glm-5") {
            request.chat_template_kwargs = Some(ChatTemplateKwargs {
                enable_thinking: Some(true),
                clear_thinking: Some(false),
            });
        }

        // Minimax models on NVIDIA NIM need temperature=1 and top_p=0.95
        if model_id.contains("minimax") {
            request.temperature = Some(1.0);
            request.top_p = Some(0.95);
        }

        request
    }
}

#[cfg(test)]
mod tests {
    use forge_domain::ModelId;
    use pretty_assertions::assert_eq;

    use super::*;

    fn create_request_fixture(model: &str) -> Request {
        Request::default()
            .model(ModelId::new(model))
            .temperature(0.7)
            .top_p(0.8)
    }

    #[test]
    fn test_glm5_sets_chat_template_kwargs() {
        let fixture = create_request_fixture("z-ai/glm-5.1");
        let mut transformer = SetNvidiaParams;
        let actual = transformer.transform(fixture);

        let kwargs = actual.chat_template_kwargs.unwrap();
        assert_eq!(kwargs.enable_thinking, Some(true));
        assert_eq!(kwargs.clear_thinking, Some(false));
    }

    #[test]
    fn test_glm5_turbo_sets_chat_template_kwargs() {
        let fixture = create_request_fixture("z-ai/glm-5-turbo");
        let mut transformer = SetNvidiaParams;
        let actual = transformer.transform(fixture);

        let kwargs = actual.chat_template_kwargs.unwrap();
        assert_eq!(kwargs.enable_thinking, Some(true));
        assert_eq!(kwargs.clear_thinking, Some(false));
    }

    #[test]
    fn test_minimax_on_nvidia_sets_temperature_and_top_p() {
        let fixture = create_request_fixture("minimaxai/minimax-m2.7");
        let mut transformer = SetNvidiaParams;
        let actual = transformer.transform(fixture);

        assert_eq!(actual.temperature, Some(1.0));
        assert_eq!(actual.top_p, Some(0.95));
    }

    #[test]
    fn test_non_nvidia_model_unchanged() {
        let fixture = create_request_fixture("gpt-4");
        let mut transformer = SetNvidiaParams;
        let actual = transformer.transform(fixture.clone());

        assert_eq!(actual.chat_template_kwargs, None);
        assert_eq!(actual.temperature, fixture.temperature);
        assert_eq!(actual.top_p, fixture.top_p);
    }

    #[test]
    fn test_no_model_unchanged() {
        let fixture = Request::default().temperature(0.7).top_p(0.8);
        let mut transformer = SetNvidiaParams;
        let actual = transformer.transform(fixture.clone());

        assert_eq!(actual.chat_template_kwargs, None);
        assert_eq!(actual.temperature, fixture.temperature);
        assert_eq!(actual.top_p, fixture.top_p);
    }
}
