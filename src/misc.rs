use crate::ConversionType;

const rational_param_array_i:[f64;6] = [
    0.0,
    1.0,
    0.0,
    0.0,
    0.0,
    1.0,
];

const param_linear_array_i:[f64;2] = [
    0.0,
    1.0,
];

pub(crate) fn transform_params(params:Vec<f64>,conv_type:ConversionType) -> Vec<f64>{
    match conv_type {
        ConversionType::Rational => {
            if params == rational_param_array_i {
                todo!()
            }else {
                todo!()
            }
        }
        _ => params
    }
}