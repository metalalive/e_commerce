import * as toolkit from "/static/js/toolkit.js";
import {append_new_form, get_default_form_props} from "./QuotaUsageTypeCommon.js";



function add_edit_forms(props) {
    // create first form, right after API data is loaded
    var forms = props.forms;
    var data  = props.data;
    var df_props = [];
    if(data) {
        for(var idx = 0; idx < data.length; idx++) {
            df_props[idx] = get_default_form_props(forms) ;
            df_props[idx].enable_close_btn = false;
            df_props[idx].btns.addform.enable = false;
            df_props[idx].form.id.defaultValue    = data[idx].id;
            df_props[idx].form.label.defaultValue = data[idx].label;
            var df_appname  = [data[idx].appname];
            var sublist = forms.app_models_list[data[idx].appname];
            if(sublist.length > 0 && sublist[0].model) {
                sublist.map((x) => { x.value = x.model; delete x.model; return x;});
            }
            var df_material = [];
            for(var jdx = 0; jdx < sublist.length; jdx++) {
                if(sublist[jdx].id == data[idx].material) {
                    df_material.push(sublist[jdx]);
                    break;
                }
            }
            df_props[idx].form.appname.defaultValue  = JSON.stringify(df_appname);
            df_props[idx].form.material.defaultValue = JSON.stringify(df_material);
            df_props[idx].form.material.whitelist = sublist;
        }
        append_new_form(forms, df_props);
    }
}


function on_page_load() {
    const _web_base_url = 'http://localhost:8006';
    const _api_base_url = 'http://localhost:8007';
    var form_layout    = document.getElementById('form-layout');

    const init_data = JSON.parse(document.getElementById('form_init_data').textContent);
    toolkit.ReactBaseForm.non_field_errors_name = 'non_field_errors';
    var submit_uri         = _api_base_url+ "/usermgt/quota";
    var success_redirect_uri = _web_base_url + "/usermgt/dashboard";
    
    // load content type data for the form set
    form_layout.app_models_list = init_data.material_type;
    form_layout.apps_list     = Object.keys(init_data.material_type);

    add_edit_forms({forms: form_layout, data:init_data.data,});

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

