use crate::ConversionType;

pub const rational_param_array_i: [f64; 6] = [0.0, 1.0, 0.0, 0.0, 0.0, 1.0];

pub const param_linear_array_i: [f64; 2] = [0.0, 1.0];

pub(crate) fn transform_params(params: Vec<f64>, conv_type: Option<ConversionType>) -> Vec<f64> {
    let mut params = params;
    match conv_type {
        Some(ConversionType::Rational) => {
            if itertools::equal(&params, &rational_param_array_i) {
                params
            } else {
                params.iter_mut().enumerate().map(|(i, x)| {
                    if *x != 0.0 {
                        *x = if i == 2 { -(*x) } else { 1.0 / (*x) };
                    }
                });
                params
            }
        }
        Some(ConversionType::ParametricLinear) => {
            if itertools::equal(&params, &param_linear_array_i) {
                params
            } else {
                params
            }
        }
        _ => params,
    }
}
