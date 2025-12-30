use slint::{ModelRc, SharedString, VecModel};

pub fn string_vec_to_rc(vec: &Vec<String>) -> ModelRc<SharedString> {
    let shared_voices: Vec<SharedString> = vec.into_iter().map(SharedString::from).collect();
    let vec_model = VecModel::from(shared_voices);
    ModelRc::new(vec_model)
}