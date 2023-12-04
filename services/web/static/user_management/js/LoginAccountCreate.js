import * as toolkit from "/static/js/toolkit.js";
import {append_new_form, get_default_form_props} from "./LoginAccountCommon.js";

var init_data = null;


function on_page_load() {
    const _web_base_url = 'http://localhost:8006';
    const _api_base_url = 'http://localhost:8007';
    var form_layout = document.getElementById('form-layout');
    form_layout.dataset.is_single_form = true;
    
    var submit_btn  = document.getElementById('submit-forms-button');
    
    init_data = JSON.parse(document.getElementById('form_init_data').textContent);
    toolkit.ReactBaseForm.non_field_errors_name =  'non_field_errors';

    var submit_uri           = _api_base_url +"/usermgt/account/create";
    var success_redirect_uri = _web_base_url +"/login";
    var api_url = [submit_uri, init_data.activate_token];
    submit_btn.forms = form_layout;
    submit_btn.dataset.api_mthd = 'POST';
    submit_btn.dataset.mode = 'cors';
    submit_btn.dataset.credentials = 'include';
    submit_btn.dataset.api_base_url = api_url.join('/');
    submit_btn.dataset.success_url_redirect = success_redirect_uri;
    submit_btn.query_params = {};
    submit_btn.addEventListener('click', toolkit.on_submit_forms);

    var props = get_default_form_props(form_layout);
    props.btns.addform.enable = false;
    props.enable_close_btn = false;
    props.form['old_uname'].disabled = true;
    props.form['old_passwd'].disabled = true;
    append_new_form(form_layout, [props]);
}

window.onload = on_page_load();

