import * as toolkit from "/static/js/toolkit.js";
import {load_api_permission_cb, append_new_form, get_default_form_props} from "./AuthRoleCommon.js";


function add_default_form_cb(avail_data, props) {
    // create first form, right after API data is loaded
    var forms = props.forms;
    var roles = props.roles_data;
    var df_props = [];
    if(roles) {
        for(var idx = 0; idx < roles.length; idx++) {
            df_props[idx] = get_default_form_props(forms) ;
            df_props[idx].enable_close_btn = false;
            df_props[idx].btns.addform.enable = false;
            df_props[idx].form.id.defaultValue   = roles[idx].id;
            df_props[idx].form.name.defaultValue = roles[idx].name;
            var opts = roles[idx].permissions.map((x) => x.name);
            df_props[idx].form.permissions.defaultValue = opts;
        }
        append_new_form(forms, df_props);
    }
}


function on_page_load() {
    var form_layout    = document.getElementById('form-layout');

    const init_data = JSON.parse(document.getElementById('form_init_data').textContent);
    toolkit.ReactBaseForm.non_field_errors_name = init_data.non_field_errors;

    var permission_api = null;
    const req_opt = {method:"GET", headers:{'accept':'application/json'},};
    const query_params = {fields:'id,name'};

    var permission_api_uri = "/"+ init_data.app_name +"/"+ init_data.permission_api_uri;
    var submit_uri         = "/"+ init_data.app_name +"/"+ init_data.submit_uri;
    var success_redirect_uri = "/"+ init_data.app_name +"/"+ init_data.success_redirect_uri;

    permission_api = new toolkit.APIconsumer({
        api_base_url: permission_api_uri, req_opt:req_opt,
        forms: form_layout, roles_data:init_data.data,
        finish_cbs: [load_api_permission_cb, add_default_form_cb]
    });
    permission_api.start(query_params);

    var submit_btn = document.getElementById('submit-forms-button');
    submit_btn.forms = form_layout;
    submit_btn.dataset.api_mthd = 'PUT';
    submit_btn.dataset.api_base_url = submit_uri ;
    //// submit_btn.query_params = {'ids': init_data.data.map((x) => x.id).join(',')};
    submit_btn.dataset.success_url_redirect = success_redirect_uri;
    submit_btn.addEventListener('click', toolkit.on_submit_forms);
}

window.onload = on_page_load();

