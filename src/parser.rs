
use crate::data::{ArithBinOp, CompareOp};
use crate::{ast, RemovalMode};
use nom_locate::{position};

mod nc {
    pub use nom::{
        combinator::*,
        multi::*,
        sequence::*,
        branch::*,
        character::complete::*,
        bytes::complete::*,
    };
}

pub type Span<'a> = nom_locate::LocatedSpan<&'a str>;
pub type Error<'a> = nom_greedyerror::GreedyError<Span<'a>, nom::error::ErrorKind>;
pub type Parsed<'a, T> = nom::IResult<Span<'a>, T, Error<'a>>;

// main

pub fn parse(input: &str) -> Result<Vec<ast::Rule<'_>>, String> {
    let input = Span::new(input);
    match document(input) {
        Ok((_, rules)) =>
            Ok(rules),
        Err(nom::Err::Error(err)) | Err(nom::Err::Failure(err)) =>
            Err(nom_greedyerror::convert_error(input, err)),
        Err(nom::Err::Incomplete(_)) =>
            panic!("unexpected incomplete parse"),
    }
}

pub fn is_variable_ident(input: &str) -> bool {
    let input = Span::new(input);
    nc::complete(nc::all_consuming(ident))(input).is_ok()
}

pub fn is_path(input: &str) -> bool {
    let input = Span::new(input);
    nc::complete(nc::all_consuming(path))(input).is_ok()
}

fn document(input: Span<'_>) -> Parsed<'_, Vec<ast::Rule<'_>>> {
    nc::complete(nc::all_consuming(
        nc::preceded(
            nc::opt(ws_or_comment),
            nc::many0(wsc_after(rule)),
        ),
    ))(input)
}

// non-significant parses

fn comment_sl(input: Span<'_>) -> Parsed<'_, ()> {
    nc::value((), nc::pair(nc::tag("//"), nc::is_not("\r\n")))(input)
}

fn comment_ml(input: Span<'_>) -> Parsed<'_, ()> {
    nc::value((), delimited_cut(nc::tag("/*"), nc::take_until("*/"), nc::tag("*/")))(input)
}

fn comment_rest(input: Span<'_>) -> Parsed<'_, ()> {
    nc::value(
        (),
        nc::pair(
            nc::verify(ident, |value| value.as_str() == "__END__"),
            nc::rest,
        ),
    )(input)
}

fn ws_or_comment(input: Span<'_>) -> Parsed<'_, ()> {
    nc::value((), nc::many1_count(nc::alt((
        comment_sl,
        comment_ml,
        comment_rest,
        nc::value((), nc::multispace1),
    ))))(input)
}

fn wsc_before<'r, R, F>(inner: F) -> impl FnMut(Span<'r>) -> Parsed<'r, R>
where
    F: FnMut(Span<'r>) -> Parsed<'r, R>,
{
    nc::preceded(nc::opt(ws_or_comment), inner)
}

fn wsc_after<'r, R, F>(inner: F) -> impl FnMut(Span<'r>) -> Parsed<'r, R>
where
    F: FnMut(Span<'r>) -> Parsed<'r, R>,
{
    nc::terminated(inner, nc::opt(ws_or_comment))
}

fn wsc<'r, R, F>(inner: F) -> impl FnMut(Span<'r>) -> Parsed<'r, R>
where
    F: FnMut(Span<'r>) -> Parsed<'r, R>,
{
    nc::delimited(nc::opt(ws_or_comment), inner, nc::opt(ws_or_comment))
}

// util

fn delimited_cut<'r, R, F, FA, FB, FAR, FBR>(
    before: FB,
    inner: F,
    after: FA,
) -> impl FnMut(Span<'r>) -> Parsed<'r, R>
where
    F: FnMut(Span<'r>) -> Parsed<'r, R>,
    FB: FnMut(Span<'r>) -> Parsed<'r, FBR>,
    FA: FnMut(Span<'r>) -> Parsed<'r, FAR>,
{
    nc::preceded(
        before,
        nc::cut(nc::terminated(
            inner,
            after,
        )),
    )
}

fn comma_sep0<'r, R, F>(inner: F) -> impl FnMut(Span<'r>) -> Parsed<'r, Vec<R>>
where
    F: FnMut(Span<'r>) -> Parsed<'r, R>,
{
    nc::terminated(
        nc::separated_list0(
            wsc(nc::char(',')),
            inner,
        ),
        wsc_before(nc::opt(nc::char(','))),
    )
}

fn block<'r, R, F>(inner: F) -> impl FnMut(Span<'r>) -> Parsed<'r, Vec<R>>
where
    F: FnMut(Span<'r>) -> Parsed<'r, R>,
{
    delimited_cut(
        nc::char('{'),
        wsc(comma_sep0(inner)),
        nc::char('}'),
    )
}

// ast

fn keyword<'r>(keyword: &'static str) -> impl FnMut(Span<'r>) -> Parsed<'r, ()> {
    nc::value((), nc::verify(ident, move |value| value.as_str() == keyword))
}

fn ident(input: Span<'_>) -> Parsed<'_, ast::Ident<'_>> {
    nc::map(
        nc::recognize(nc::pair(
            nc::alt((nc::alpha1, nc::tag("_"))),
            nc::many0(nc::alt((nc::alphanumeric1, nc::tag("_")))),
        )),
        |span| ast::Ident { span },
    )(input)
}

fn variable(input: Span<'_>) -> Parsed<'_, ast::Variable<'_>> {
    nc::map(
        nc::preceded(
            nc::char('$'),
            nc::opt(ident),
        ),
        |maybe_ident| maybe_ident.map(ast::Variable::Ident).unwrap_or(ast::Variable::Wildcard),
    )(input)
}

fn int(input: Span<'_>) -> Parsed<'_, i64> {
    nc::map_opt(
        nc::recognize(nc::pair(
            nc::preceded(
                nc::opt(nc::char('-')),
                nc::many1_count(nc::digit1),
            ),
            nc::many0_count(nc::pair(nc::char('_'), nc::many1_count(nc::digit1))),
        )),
        |span: Span<'_>| {
            let value = span.fragment();
            if value.contains("_") {
                value.replace("_", "").parse().ok()
            } else {
                value.parse().ok()
            }
        },
    )(input)
}

fn float(input: Span<'_>) -> Parsed<'_, f64> {
    nc::map_opt(
        nc::recognize(nc::tuple((
            nc::opt(nc::char('-')),
            nc::pair(
                nc::many1_count(nc::digit1),
                nc::many0_count(nc::pair(nc::char('_'), nc::many1_count(nc::digit1))),
            ),
            nc::char('.'),
            nc::cut(nc::pair(
                nc::many1_count(nc::digit1),
                nc::many0_count(nc::pair(nc::char('_'), nc::many1_count(nc::digit1))),
            )),
        ))),
        |span: Span<'_>| {
            let value = span.fragment();
            if value.contains("_") {
                value.replace("_", "").parse().ok()
            } else {
                value.parse().ok()
            }
        },
    )(input)
}

fn literal(input: Span<'_>) -> Parsed<'_, ast::Literal<'_>> {
    nc::alt((
        nc::map(ident, ast::Literal::Symbol),
        nc::map(float, ast::Literal::Float),
        nc::map(int, ast::Literal::Int),
    ))(input)
}

fn path(input: Span<'_>) -> Parsed<'_, ast::Path<'_>> {
    nc::map(
        nc::recognize(nc::pair(
            ident,
            nc::many0_count(
                nc::preceded(
                    nc::char('.'),
                    nc::cut(ident),
                ),
            ),
        )),
        |span| ast::Path { span },
    )(input)
}

fn rule_identity(input: Span<'_>) -> Parsed<'_, (ast::Path<'_>, ast::Path<'_>)> {
    nc::pair(
        path,
        nc::preceded(
            wsc(nc::char(':')),
            nc::cut(path),
        ),
    )(input)
}

fn comparable(input: Span<'_>) -> Parsed<'_, ast::Comparable<'_>> {
    nc::alt((
        nc::map(float, ast::Comparable::Float),
        nc::map(int, ast::Comparable::Int),
        nc::map(variable, ast::Comparable::Variable),
    ))(input)
}

fn comparison(input: Span<'_>) -> Parsed<'_, ast::Comparison<'_>> {
    nc::map(
        nc::tuple((
            position,
            comparable,
            wsc(nc::alt((
                nc::value(CompareOp::Equal, nc::tag("==")),
                nc::value(CompareOp::NotEqual, nc::tag("!=")),
                nc::value(CompareOp::LessOrEqual, nc::tag("<=")),
                nc::value(CompareOp::Less, nc::tag("<")),
                nc::value(CompareOp::GreaterOrEqual, nc::tag(">=")),
                nc::value(CompareOp::Greater, nc::tag(">")),
            ))),
            nc::cut(comparable),
        )),
        |(position, left, ordering, right)| ast::Comparison { position, ordering, left, right },
    )(input)
}

fn binding_spec(input: Span<'_>) -> Parsed<'_, ast::BindingSpec<'_>> {
    nc::map(
        nc::tuple((
            position,
            variable,
            nc::preceded(
                wsc(nc::char(':')),
                nc::cut(value_spec),
            ),
        )),
        |(position, variable, value_spec)| ast::BindingSpec { variable, value_spec, position },
    )(input)
}

fn binding_attribute_spec(input: Span<'_>) -> Parsed<'_, ast::BindingAttributeSpec<'_>> {
    nc::map(
        nc::tuple((
            position,
            variable,
            nc::preceded(
                wsc(nc::char('.')),
                nc::cut(attribute_spec),
            ),
        )),
        |(position, variable, attribute_spec)| {
            ast::BindingAttributeSpec { position, variable, attribute_spec }
        },
    )(input)
}

fn attribute_spec(input: Span<'_>) -> Parsed<'_, ast::AttributeSpec<'_>> {
    nc::map(
        nc::tuple((
            position,
            ident,
            nc::preceded(
                wsc(nc::char(':')),
                nc::cut(value_spec),
            ),
        )),
        |(position, attribute, value_spec)| ast::AttributeSpec { position, attribute, value_spec },
    )(input)
}

fn value_spec(input: Span<'_>) -> Parsed<'_, ast::ValueSpec<'_>> {
    nc::map(
        nc::pair(position, nc::alt((
            nc::flat_map(
                nc::opt(nc::terminated(variable, wsc(nc::char('@')))),
                |variable| {
                    let variable_tuple = variable.unwrap_or(ast::Variable::Wildcard);
                    let variable_enum = variable_tuple.clone();
                    let variable_struct = variable_tuple.clone();
                    nc::alt((
                        nc::map(value_spec_tuple, move |inner| ast::ValueSpecKind::Tuple(ast::Bindable {
                            variable: variable_tuple.clone(),
                            inner,
                        })),
                        nc::map(value_spec_enum, move |inner| ast::ValueSpecKind::Enum(ast::Bindable {
                            variable: variable_enum.clone(),
                            inner,
                        })),
                        nc::map(
                            block(attribute_spec),
                            move |inner| ast::ValueSpecKind::Struct(ast::Bindable {
                                variable: variable_struct.clone(),
                                inner,
                            }),
                        ),
                    ))
                },
            ),
            nc::map(variable, ast::ValueSpecKind::Variable),
            nc::map(literal, ast::ValueSpecKind::Literal),
        ))),
        |(position, kind)| ast::ValueSpec { position, kind },
    )(input)
}

fn value_spec_enumerable(input: Span<'_>) -> Parsed<'_, ast::Enumerable<'_>> {
    nc::alt((
        nc::map(literal, ast::Enumerable::Literal),
        nc::map(variable, ast::Enumerable::Variable),
    ))(input)
}

fn value_spec_enum(input: Span<'_>) -> Parsed<'_, Vec<ast::Enumerable<'_>>> {
    nc::map(
        nc::pair(
            value_spec_enumerable,
            nc::verify(
                nc::many0(nc::preceded(
                    wsc(nc::char('|')),
                    value_spec_enumerable,
                )),
                |values: &Vec<ast::Enumerable<'_>>| !values.is_empty(),
            ),
        ),
        |(first, mut rest)| {
            rest.insert(0, first);
            rest
        },
    )(input)
}

fn value_spec_tuple(input: Span<'_>) -> Parsed<'_, Vec<ast::ValueSpec<'_>>> {
    delimited_cut(
        nc::char('['),
        comma_sep0(value_spec),
        nc::char(']'),
    )(input)
}

fn calculation_add_sub(input: Span<'_>) -> Parsed<'_, ast::Calculation<'_>> {
    let (input, first) = calculation_mul_div(input)?;
    nc::fold_many0(
        nc::pair(
            wsc(nc::alt((
                nc::value(ArithBinOp::Add, nc::char('+')),
                nc::value(ArithBinOp::Sub, nc::char('-')),
            ))),
            nc::cut(calculation_mul_div),
        ),
        first,
        |left, (op, right)| ast::Calculation::BimOp(op, Box::new(left), Box::new(right)),
    )(input)
}

fn calculation_mul_div(input: Span<'_>) -> Parsed<'_, ast::Calculation<'_>> {
    let (input, first) = calculation_terminal(input)?;
    nc::fold_many0(
        nc::pair(
            wsc(nc::alt((
                nc::value(ArithBinOp::Mul, nc::char('*')),
                nc::value(ArithBinOp::Div, nc::char('/')),
            ))),
            nc::cut(calculation_terminal),
        ),
        first,
        |left, (op, right)| ast::Calculation::BimOp(op, Box::new(left), Box::new(right)),
    )(input)
}

fn calculation_terminal(input: Span<'_>) -> Parsed<'_, ast::Calculation<'_>> {
    nc::alt((
        nc::map(float, ast::Calculation::Float),
        nc::map(int, ast::Calculation::Int),
        nc::map(variable, ast::Calculation::Variable),
        delimited_cut(nc::char('('), wsc(calculation), nc::char(')')),
    ))(input)
}

fn calculation(input: Span<'_>) -> Parsed<'_, ast::Calculation<'_>> {
    calculation_add_sub(input)
}

fn rule_apply(input: Span<'_>) -> Parsed<'_, ast::RuleApply<'_>> {
    nc::alt((
        nc::map(
            nc::preceded(wsc_after(nc::char('+')), binding_attribute_spec),
            ast::RuleApply::Add,
        ),
        nc::map(
            nc::pair(
                wsc_after(nc::alt((
                    nc::value(RemovalMode::Optional, nc::tag("-?")),
                    nc::value(RemovalMode::Required, nc::tag("-")),
                ))),
                binding_attribute_spec,
            ),
            |(mode, spec)| ast::RuleApply::Remove(spec, mode),
        ),
    ))(input)
}

fn rule_select(input: Span<'_>) -> Parsed<'_, ast::RuleSelect<'_>> {
    nc::alt((
        nc::map(binding_spec, ast::RuleSelect::Binding),
        nc::map(binding_attribute_spec, ast::RuleSelect::BindingAttribute),
        nc::map(comparison, ast::RuleSelect::Comparison),
        nc::map(
            nc::tuple((
                position,
                variable,
                nc::preceded(
                    wsc(keyword("is")),
                    nc::cut(calculation),
                ),
            )),
            |(position, variable, calc)| ast::RuleSelect::Calculation(variable, calc, position),
        ),
        nc::map(
            nc::preceded(
                wsc_after(keyword("not")),
                nc::cut(block(rule_select)),
            ),
            ast::RuleSelect::Not,
        ),
    ))(input)
}

fn rule(input: Span<'_>) -> Parsed<'_, ast::Rule<'_>> {
    nc::map(
        nc::preceded(
            wsc_after(keyword("rule")),
            nc::cut(nc::tuple((
                wsc_after(rule_identity),
                block(rule_select),
                nc::preceded(
                    wsc(keyword("do")),
                    block(rule_apply),
                ),
            ))),
        ),
        |((system_name, name), select, apply)| ast::Rule {
            system_name,
            name,
            select,
            apply,
        },
    )(input)
}
