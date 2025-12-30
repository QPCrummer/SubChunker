use slint::{ModelRc, SharedString, VecModel};

pub fn string_vec_to_rc(vec: &Vec<String>) -> ModelRc<SharedString> {
    let shared_voices: Vec<SharedString> = vec.into_iter().map(SharedString::from).collect();
    let vec_model = VecModel::from(shared_voices);
    ModelRc::new(vec_model)
}

pub fn string_arr_to_rc(arr: &[&str]) -> ModelRc<SharedString> {
    let shared_voices: Vec<SharedString> = arr.into_iter().map(|x| {
        SharedString::from(*x)
    }).collect();
    let vec_model = VecModel::from(shared_voices);
    ModelRc::new(vec_model)
}

pub fn bool_arr_to_rc(arr: &[bool]) -> ModelRc<bool> {
    let shared_voices: Vec<bool> = arr.into_iter().map(|x| {
        *x
    }).collect();
    let vec_model = VecModel::from(shared_voices);
    ModelRc::new(vec_model)
}
