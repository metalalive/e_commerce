import * as toolkit from "/static/js/toolkit.js";
import * as quotaform from "./quota_form.js";
import {load_api_data,} from "./UserGroupsCommon.js";


function on_page_load() {
    const _web_base_url = 'http://localhost:8006';
    const _api_base_url = 'http://localhost:8007';
    var form_layout    = document.getElementById('form-layout');

    const init_data = JSON.parse(document.getElementById('form_init_data').textContent);
    toolkit.ReactBaseForm.non_field_errors_name = 'non_field_errors';
    
    var submit_uri           = _api_base_url +"/usermgt/usrgrps";
    var success_redirect_uri = _web_base_url +"/usermgt/dashboard";
    
    var authrole_api_uri     = _api_base_url +"/usermgt/roles";
    var quotatype_api_uri    = _api_base_url +"/usermgt/quota";
    var usrgrp_api_uri       = _api_base_url +"/usermgt/usrgrps";
    var api_uris = {authrole: authrole_api_uri, quotatype: quotatype_api_uri, usrgrp: usrgrp_api_uri,};
    load_api_data(form_layout, api_uris);

    var submit_btn = document.getElementById('submit-forms-button');
    submit_btn.forms = form_layout;
    submit_btn.dataset.api_mthd = 'POST';
    submit_btn.dataset.mode = 'cors';
    submit_btn.dataset.credentials = 'include';
    submit_btn.dataset.api_base_url = submit_uri;
    submit_btn.dataset.success_url_redirect = success_redirect_uri;
    submit_btn.addEventListener('click', toolkit.on_submit_forms);
}

window.onload = on_page_load();

