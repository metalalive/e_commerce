import * as toolkit from "/static/js/toolkit.js";
import {load_api_data,} from "./UsersProfCommon.js";


function on_page_load() {
    var form_layout    = document.getElementById('form-layout');

    const init_data = JSON.parse(document.getElementById('form_init_data').textContent);
    toolkit.ReactBaseForm.non_field_errors_name = init_data.non_field_errors;

    var submit_uri           = "/"+ init_data.app_name +"/"+ init_data.submit_uri;
    var success_redirect_uri = "/"+ init_data.app_name +"/"+ init_data.success_redirect_uri;

    var authrole_api_uri     = "/"+ init_data.app_name +"/"+ init_data.authrole_api_uri;
    var quotatype_api_uri    = "/"+ init_data.app_name +"/"+ init_data.quotatype_api_uri;
    var usrgrp_api_uri       = "/"+ init_data.app_name +"/"+ init_data.usrgrp_api_uri;
    var api_uris = {authrole: authrole_api_uri, quotatype: quotatype_api_uri, usrgrp: usrgrp_api_uri,};
    load_api_data(form_layout, null, api_uris);

    // add event listener to add-form button laid in HTML file.
    var submit_btn = document.getElementById('submit-forms-button');
    submit_btn.forms = form_layout;
    submit_btn.dataset.api_mthd = 'POST';
    submit_btn.dataset.api_base_url = submit_uri;
    submit_btn.dataset.success_url_redirect = success_redirect_uri;
    submit_btn.addEventListener('click', toolkit.on_submit_forms);
}


window.onload = on_page_load();

