import * as toolkit from "/static/js/toolkit.js";
import * as loginform from "./LoginAccountCommon.js";

var init_data = null;


function on_page_load() {
    var form_layout = document.getElementById('form-layout');
    form_layout.dataset.is_single_form = true;
    
    var submit_btn  = document.getElementById('submit-forms-button');
    init_data = JSON.parse(document.getElementById('form_init_data').textContent);
    toolkit.ReactBaseForm.non_field_errors_name = init_data.non_field_errors;

    submit_btn.forms = form_layout;
    submit_btn.dataset.api_mthd = 'POST';
    submit_btn.dataset.mode = 'cors';
    submit_btn.dataset.credentials = 'include';
    submit_btn.dataset.api_base_url = init_data.submit_url;
    submit_btn.dataset.success_url_redirect = init_data.redirect;
    submit_btn.query_params = {};
    submit_btn.addEventListener('click', toolkit.on_submit_forms);
    var props = loginform.get_default_form_props(form_layout);
    props.btns.addform.enable = false;
    props.enable_close_btn = false;
    props.form['password2'].disabled = true;
    props.form['old_uname'].disabled = true;
    props.form['old_passwd'].disabled = true;

    loginform.append_new_form(form_layout, [props]);
}

window.onload = on_page_load();

