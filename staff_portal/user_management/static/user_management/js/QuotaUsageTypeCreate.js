import * as toolkit from "/static/js/toolkit.js";
import {append_new_form, get_default_form_props} from "./QuotaUsageTypeCommon.js";


function on_page_load() {
    var form_layout    = document.getElementById('form-layout');

    const init_data = JSON.parse(document.getElementById('form_init_data').textContent);
    toolkit.ReactBaseForm.non_field_errors_name = init_data.non_field_errors;
    
    var submit_uri         = "/"+ init_data.app_name +"/"+ init_data.submit_uri;
    var success_redirect_uri = "/"+ init_data.app_name +"/"+ init_data.success_redirect_uri;
    
    // load content type data for the form set
    form_layout.app_models_list = init_data.material_type;
    form_layout.apps_list     = Object.keys(init_data.material_type);

    // create first form
    var df_props = [];
    df_props[0] = get_default_form_props(form_layout);
    df_props[0].enable_close_btn = false;
    append_new_form(form_layout, df_props);

    var submit_btn = document.getElementById('submit-forms-button');
    submit_btn.forms = form_layout;
    submit_btn.dataset.api_mthd = 'POST';
    submit_btn.dataset.api_base_url = submit_uri;
    submit_btn.dataset.success_url_redirect = success_redirect_uri;
    submit_btn.addEventListener('click', toolkit.on_submit_forms);
}

window.onload = on_page_load();

