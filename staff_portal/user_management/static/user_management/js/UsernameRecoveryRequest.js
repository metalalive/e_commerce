import * as toolkit from "/static/js/toolkit.js";
import * as emailform from "./email_form.js";

var init_data = null;


function on_page_load() {
    var form_layout = document.getElementById('form-layout');
    form_layout.dataset.is_single_form = true;
    
    var submit_btn  = document.getElementById('submit-forms-button');
    init_data = JSON.parse(document.getElementById('form_init_data').textContent);

    submit_btn.forms = form_layout;
    submit_btn.dataset.api_mthd = 'POST';
    submit_btn.dataset.api_base_url = "/usermgt/username/recovery";
    submit_btn.dataset.success_url_redirect = "/usermgt/login";
    submit_btn.query_params = {};
    submit_btn.addEventListener('click', toolkit.on_submit_forms);
    var props = emailform.get_default_form_props(form_layout);
    props.btns.addform.enable = false;
    props.enable_close_btn = false;
    emailform.append_new_form(form_layout, [props]);
}

window.onload = on_page_load();

