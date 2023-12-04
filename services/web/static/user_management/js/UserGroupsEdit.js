import * as toolkit from "/static/js/toolkit.js";
import * as quotaform from "./quota_form.js";
import {get_default_form_props, usrgrp_seeker, role_seeker, load_api_data} from "./UserGroupsCommon.js";

var init_data = null;
var req_opt = null;


function  load_edit_data_cb(container) {
    // create first form, right after all necessary API data is loaded
    var df_props = [];
    var prop = null;
    var datarow = null;
    for (var idx = 0; idx < init_data.data.length; idx++) {
        datarow = init_data.data[idx];
        prop = get_default_form_props(container) ;
        prop.enable_close_btn = false;
        prop.btns.addform.enable = false;
        prop.form['id'].defaultValue  = datarow.id;
        prop.form['name'].defaultValue  = datarow.name;
        prop.form['new_parent'].hidden  = true;
        var exist_parent = [];
        if(datarow.ancestors.length == 1) {
            var parent_id = datarow.ancestors[0].ancestor.id ;
            var parent_grp = usrgrp_seeker[parent_id];
            if(parent_grp) {
                exist_parent.push(parent_grp);
            }
        }
        prop.form['exist_parent'].defaultValue = JSON.stringify(exist_parent);
        //// var roles = datarow.roles.map((x) =>  { return  role_seeker[x.id];});
        var roles = datarow.roles.filter(x => role_seeker[x.id]).map(x => role_seeker[x.id]);
        prop.form['roles'].defaultValue = JSON.stringify(roles);
        prop.form['quota'].defaultValue = quotaform.render_with_data(datarow.quota);
        df_props.push(prop);
    } // end of outer loop
    return df_props;
}


function on_page_load() {
    const _web_base_url = 'http://localhost:8006';
    const _api_base_url = 'http://localhost:8007';
    var form_layout = document.getElementById('form-layout');
    form_layout.load_edit_data_cb = load_edit_data_cb;

    init_data = JSON.parse(document.getElementById('form_init_data').textContent);
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
    submit_btn.dataset.api_mthd = 'PUT';
    submit_btn.dataset.mode = 'cors';
    submit_btn.dataset.credentials = 'include';
    submit_btn.dataset.api_base_url = submit_uri;
    submit_btn.dataset.success_url_redirect = success_redirect_uri;
    submit_btn.addEventListener('click', toolkit.on_submit_forms);
}

window.onload = on_page_load();

