import * as toolkit from "/static/js/toolkit.js";
import * as emailform from "./email_form.js";

var init_data = null;


function on_page_load() {
    const _web_base_url = 'http://localhost:8006';
    const _api_base_url = 'http://localhost:8007';
    var form_layout = document.getElementById('form-layout');
    form_layout.dataset.is_single_form = true;
    
    var submit_btn  = document.getElementById('submit-forms-button');
    init_data = JSON.parse(document.getElementById('form_init_data').textContent);
    toolkit.ReactBaseForm.non_field_errors_name =  'non_field_errors';

    submit_btn.forms = form_layout;
    submit_btn.dataset.api_mthd = 'POST';
    submit_btn.dataset.mode = 'cors';
    submit_btn.dataset.credentials = 'include';
    submit_btn.dataset.api_base_url = _api_base_url + "/usermgt/password/reset";
    submit_btn.dataset.success_url_redirect = _web_base_url + "/login";
    submit_btn.query_params = {};
    submit_btn.addEventListener('click', toolkit.on_submit_forms);
    var props = emailform.get_default_form_props(form_layout);
    props.btns.addform.enable = false;
    props.enable_close_btn = false;
    emailform.append_new_form(form_layout, [props]);
}

window.onload = on_page_load();

