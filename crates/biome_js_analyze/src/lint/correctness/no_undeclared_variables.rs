use crate::globals::{is_js_global, is_ts_global};
use crate::services::semantic::SemanticServices;
use biome_analyze::context::RuleContext;
use biome_analyze::options::JsxRuntime;
use biome_analyze::{declare_lint_rule, Rule, RuleDiagnostic, RuleSource};
use biome_console::markup;
use biome_js_syntax::{
    AnyJsFunction, JsFileSource, Language, TextRange, TsAsExpression, TsReferenceType,
};
use biome_rowan::AstNode;

const REACT_JSX_FACTORY: &str = "React";

declare_lint_rule! {
    /// Prevents the usage of variables that haven't been declared inside the document.
    ///
    /// If you need to allow-list some global bindings, you can use the [`javascript.globals`](/reference/configuration/#javascriptglobals) configuration.
    ///
    /// ## Examples
    ///
    /// ### Invalid
    ///
    /// ```js,expect_diagnostic
    /// foobar;
    /// ```
    ///
    /// ```js,expect_diagnostic
    /// // throw diagnostic for JavaScript files
    /// PromiseLike;
    /// ```
    /// ### Valid
    ///
    /// ```ts
    /// type B<T> = PromiseLike<T>
    /// ```
    pub NoUndeclaredVariables {
        version: "1.0.0",
        name: "noUndeclaredVariables",
        language: "js",
        sources: &[RuleSource::Eslint("no-undef")],
        recommended: false,
    }
}

impl Rule for NoUndeclaredVariables {
    type Query = SemanticServices;
    type State = (TextRange, String);
    type Signals = Vec<Self::State>;
    type Options = ();

    fn run(ctx: &RuleContext<Self>) -> Self::Signals {
        ctx.query()
            .all_unresolved_references()
            .filter_map(|reference| {
                if let Some(identifier) = reference.as_js_identifier() {
                    let under_as_expression = identifier
                        .parent::<TsReferenceType>()
                        .and_then(|ty| ty.parent::<TsAsExpression>())
                        .is_some();

                    let token = identifier.value_token().ok()?;
                    let text = token.text_trimmed();

                    let source_type = ctx.source_type::<JsFileSource>();

                    if ctx.is_global(text) {
                        return None;
                    }

                    // Typescript Const Assertion
                    if text == "const" && under_as_expression {
                        return None;
                    }

                    // arguments object within non-arrow functions
                    if text == "arguments" {
                        let is_in_non_arrow_function =
                            identifier.syntax().ancestors().any(|ancestor| {
                                !matches!(
                                    AnyJsFunction::cast(ancestor),
                                    None | Some(AnyJsFunction::JsArrowFunctionExpression(_))
                                )
                            });
                        if is_in_non_arrow_function {
                            return None;
                        }
                    }

                    if is_global(text, source_type) {
                        return None;
                    }

                    let span = token.text_trimmed_range();
                    let text = text.to_string();
                    Some((span, text))
                } else if ctx.jsx_runtime() == JsxRuntime::ReactClassic {
                    if let Some(jsx_like) = reference.as_jsx_like() {
                        let jsx_factory = ctx.jsx_factory()?;
                        if jsx_factory == REACT_JSX_FACTORY {
                            return None;
                        }
                        let span = jsx_like.name_value_token()?.text_trimmed_range();
                        return Some((span, jsx_factory.to_string()));
                    }

                    if let Some(jsx_fragment) = reference.as_jsx_fragment() {
                        let jsx_fragment_factory = ctx.jsx_fragment_factory()?;
                        if jsx_fragment_factory == REACT_JSX_FACTORY {
                            return None;
                        }
                        let span = jsx_fragment.l_angle_token().ok()?.text_trimmed_range();
                        return Some((span, jsx_fragment_factory.to_string()));
                    }

                    None
                } else {
                    None
                }
            })
            .collect()
    }

    fn diagnostic(_ctx: &RuleContext<Self>, (span, name): &Self::State) -> Option<RuleDiagnostic> {
        Some(RuleDiagnostic::new(
            rule_category!(),
            *span,
            markup! {
                "The "<Emphasis>{name}</Emphasis>" variable is undeclared."
            },
        ).note(markup! {
            "By default, Biome recognizes browser and Node.js globals.\nYou can ignore more globals using the "<Hyperlink href="https://biomejs.dev/reference/configuration/#javascriptglobals">"javascript.globals"</Hyperlink>" configuration."
        }))
    }
}

fn is_global(reference_name: &str, source_type: &JsFileSource) -> bool {
    match source_type.language() {
        Language::JavaScript => is_js_global(reference_name),
        Language::TypeScript { .. } => is_js_global(reference_name) || is_ts_global(reference_name),
    }
}
