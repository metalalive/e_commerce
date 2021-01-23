
import * as toolkit from "/static/js/toolkit.js";
import {update_pagination_cb, init_paging_panel} from "/static/js/pagination.js";
import {init_order_btns} from "/static/js/ordering.js";
import {init_search_bar} from "/static/js/searchbar.js";
import * as emailform from "/static/user_management/js/email_form.js"
import * as login_account  from "./LoginAccountCommon.js";
import * as activitylog  from "./ActivityLog.js";

const common_req_opt = {method:"GET", headers:{'accept':'application/json'},};
var alert_prompt_ctrl = null;
var global_tabs = null;
var global_templates = null;
var uri_dict = null;


function run_action_bulk(evt)
{
    var target    = evt.target;
    var record_list = target.api.record_list;
    var cls_name = target.dataset.cls_name;
    var command  = target.dataset.command;
    var base_url = target.dataset.base_url;
    var idx = 0;
    var in_elms = Array.from(record_list.querySelectorAll("input[id="+ cls_name +"]:checked"));
    var ids = in_elms.map(x => x.value);

    if(command == 'edit') {
        var url_query = "?ids="+ ids.join(",");
        var extra_query_params = target.dataset.extra_query_params;
        if(extra_query_params) {
            url_query += "&"+ extra_query_params;
        }
        if(ids.length > 0) {
            url_query = base_url + url_query;
            window.location.href = url_query;
        }
    } else if(command == 'del') {
        if(ids.length > 0) {
            var props = {};
            props.url = base_url;
            props.req_opt = {method:"DELETE", headers:{'accept':'application/json'},};
            props.query_params = {'ids': ids.join(",")};
            props.caller = target;
            props.callbacks = [prompt_api_resp_msg, api_delete_items_cb, ];
            perform_api_call(props);
        }
    }
} // end of  run_action_bulk


function evt_switch_tab(evt) {
    var tabs = evt.target.tabs;
    if(tabs == undefined) { return; }
    for(var key in tabs) {
        tabs[key].dom.classList.remove('active');
    }
    var target_tab_id = null;
    if(evt.target.dataset && evt.target.dataset.substitute_id) {
        target_tab_id = evt.target.dataset.substitute_id;
    } else {
        target_tab_id = evt.target.id;
    }
    var visible_tab = tabs[target_tab_id];
    //// if(visible_tab == undefined) { return; }
    var visible_dom = visible_tab.dom;
    //// if(visible_dom == undefined) { return; }
    visible_tab.init_fn(visible_tab, global_templates);
    visible_dom.classList.add('active');
}

function evt_change_tabbtn_bg(evt) {
    var tab_btns = evt.target.tab_btns;
    for(var idx = 0; idx < tab_btns.length; idx++) {
        tab_btns[idx].classList.remove('active');
    }
    evt.target.classList.add('active');
}

function run_recovery(evt) {
    var target   = evt.target;
    var base_url = target.dataset.base_url;
    var props = {};
    var body_data = {time_start:target.dataset.time_start};
    body_data = JSON.stringify(body_data);
    props.url = base_url;
    props.req_opt = {method:"PATCH", body:body_data,
        headers:{'content-type':'application/json', 'accept':'application/json'},};
    props.caller = target;
    props.callbacks = [prompt_api_resp_msg];
    perform_api_call(props);
} // end of run_recovery


function perform_logout(evt) {
    var target   = evt.target;
    var base_url = target.dataset.base_url;
    var props = {};
    props.url = base_url;
    props.req_opt = {method:"POST", body:"", headers:{'content-type':'application/json', 'accept':'application/json'},};
    props.caller = target;
    props.callbacks = [prompt_api_resp_msg];
    perform_api_call(props);
}


function apply_loglevel_change(evt) {
    var target   = evt.target;
    var base_url = target.dataset.base_url;
    var props = {};
    var method = '';
    var new_level = null;
    if(target.level_field) {
        method = 'PUT';
        new_level = target.level_field.value;
    } else {
        method = 'DELETE';
    }
    var body_data = [{name: target.dataset.logger_name, level: new_level}];
    body_data = JSON.stringify(body_data);
    props.req_opt = {method: method, body:body_data,
        headers:{'content-type':'application/json', 'accept':'application/json'},};
    props.url = base_url;
    props.caller = target;
    props.callbacks = [prompt_api_resp_msg];
    perform_api_call(props);
}


function activate_login_account(kwargs) {
    var users = kwargs.users.map((x) => {
        x.profile = x.id;
        delete x.id;
        var m = kwargs._chosen_emails.find((y) => y.user_id == x.profile);
        if(m) { x.email = m.email; }
        return x;
    });
    users = users.filter(x => x.email);
    var url = uri_dict['UserActivationView'];
    var caller = {dataset: {base_url:url},};
    var props  = {url: url+"?fields=profile", caller:caller, callbacks: [prompt_api_resp_msg]};
    props.req_opt = {method:"POST", body: JSON.stringify(users),
        headers:{'content-type':'application/json', 'accept':'application/json'},};
    perform_api_call(props);
}

function deactivate_login_account(kwargs) {
    var users = kwargs.users;
    var url = uri_dict['UserDeactivationView'];
    var caller = {dataset: {base_url:url},};
    var props  = {url: url, caller:caller, callbacks: [prompt_api_resp_msg]};
    props.req_opt = {method:"POST", body: JSON.stringify(users),
        headers:{'content-type':'application/json', 'accept':'application/json'},};
    perform_api_call(props);
}


function  dismiss_activation_prompt(kwargs) {
    var caller = kwargs.caller;
    caller.checked = false;
    console.log("dismiss the change on activation...");
}



function toggle_auth_activation(evt) {
    var target  = evt.target;
    var user_id = target.dataset.user_id;
    var first_name = target.dataset.first_name;
    var last_name  = target.dataset.last_name;
    var kwargs = {};

    if(target.checked) { // activate users and their login account
        var user_item = {id:user_id, first_name:first_name, last_name:last_name};
        kwargs.users = [user_item];
        var fetch_usr_fields = ['id', 'emails', 'email', 'addr',];
        kwargs.api = {req_opt: common_req_opt, url:uri_dict['UserProfileAPIView'][0], load_api_fn:perform_api_call,
            callbacks:null, caller:target, query_params:{ordering:'id', fields: fetch_usr_fields.join(',')},
        };  // forcalling API to fetch user emails
        kwargs.modal = {
            title: "Choose email from the following user(s)",
            error_message: "There is no email for user activation, please add email for the user(s) first",
            btns:{
                apply: {label: "Send activation mail", callback: {fn:activate_login_account, kwargs: {users: [user_item],}}},
                close: {callback:   {fn:dismiss_activation_prompt, kwargs: {caller: target}}},
                dismiss: {callback: {fn:dismiss_activation_prompt, kwargs: {caller: target}}},
            },
        };
        emailform.launch_mail_selector(kwargs);
    } else { // deactivate users and their login account
        var user_item = {id:user_id, remove_account:false};
        kwargs.users = [user_item];
        deactivate_login_account(kwargs);
    }
} // end of toggle_auth_activation


function hier_compare_fn(a,b) {
    var out = 0;
    if(a.depth > b.depth) {
        out = 1;
    } else if(a.depth < b.depth) {
        out = -1;
    }
    return out;
}


function render_hierarchy_text(container, data, props) {
    var hier_names = null;
    var substr_len = props.max_len.hierarchy;
    data = data.sort(hier_compare_fn).reverse();
    hier_names = data.map(x => encodeURIComponent(x.ancestor.name))
    hier_names = hier_names.join('/');
    // TODO, find better solution, this is quick-and-dirty way to avoid URI decode error
    do {
        try {
            var tmp = toolkit.truncate_text(hier_names, substr_len++);
            tmp = tmp.split('/');
            tmp = tmp.map(x => decodeURIComponent(x));
            hier_names = tmp;
            break
        } catch(err) {
            console.log("need to adjust truncated encoded string : "+ substr_len);
        }
    } while(substr_len < hier_names.length);

    container.innerHTML = "";
    for(var idx = 0; idx < hier_names.length; idx++) {
        if(data[idx] == undefined) { break; }
        //// var link = "<a href='"+ props.detail_url + data[idx].ancestor.id +"'>"+ hier_names[idx] +"</a>";
        //// out.push(link);
        var _link_dom = document.createElement("a");
        var delimiter = document.createElement("b");
        delimiter.innerHTML = "/";
        _link_dom.href = "#";
        _link_dom.classList.add('p-1');
        _link_dom.innerText = hier_names[idx];
        render_detail_tab_btn(_link_dom, props.detail, data[idx].ancestor.id);
        container.appendChild(_link_dom);
        container.appendChild(delimiter);
    }
} // end of render_hierarchy_text


function _get_data_row_dom(data_table, idx) {
    var out = null;
    if((idx + 2) == data_table.children.length) {
        out = data_table.children[1].cloneNode(true);
        data_table.appendChild(out);
    } else {
        out = data_table.children[idx + 2];
    }
    out.hidden = false;
    return out;
}


function load_api_listusrgrp_cb(avail_data, props) {
    if (!avail_data.results) {
        return;
    }
    var idx = 0;
    var record_list = props.record_list;
    var table_dom = record_list.querySelector("table").children[0]; // auto generated tbody
    for(idx = 0; idx < avail_data.results.length; idx++) {
        var record = _get_data_row_dom(table_dom, idx);
        var chkbox_field = record.querySelector("input[id=ugid]");
        var name_field   = record.querySelector("td[id=name]");
        var hierarchy_field = record.querySelector("td[id=hierarchy]");
        var usrs_cnt_field  = record.querySelector("td[id=usr_cnt]");
        chkbox_field.value  = avail_data.results[idx].id;
        chkbox_field.checked = false;
        name_field.children[0].innerText = toolkit.truncate_text(avail_data.results[idx].name, props.max_len.name);
        render_detail_tab_btn(name_field.children[0], props.detail, avail_data.results[idx].id);
        render_hierarchy_text(hierarchy_field, avail_data.results[idx].ancestors, props);
        usrs_cnt_field.innerText  = avail_data.results[idx].usr_cnt;
    } // end of data renderring in rows
    for(idx += 2; idx < table_dom.children.length; idx++) {
        table_dom.children[idx].hidden = true;
    }
} // end of load_api_listusrgrp_cb


function load_api_listusrprof_cb(avail_data, props) {
    if (!avail_data.results) {
        return;
    }
    var record_list = props.record_list;
    var table_dom = record_list.querySelector("table").children[0]; // auto generated tbody
    for(var idx = 0; idx < avail_data.results.length; idx++) {
        var record = _get_data_row_dom(table_dom, idx);
        var chkbox_field = record.querySelector("input[id=upid]");
        var first_name_field  = record.querySelector("td[id=first_name]");
        var last_name_field   = record.querySelector("td[id=last_name]");
        var time_joined_field = record.querySelector("td[id=date_joined]");
        var last_updated_field= record.querySelector("td[id=last_updated]");
        var can_login_field   = record.querySelector("td[id=can_login]");
        chkbox_field.value  = avail_data.results[idx].id;
        chkbox_field.checked = false;
        first_name_field.children[0].innerText = toolkit.truncate_text(avail_data.results[idx].first_name, props.max_len.first_name);
        last_name_field.children[0].innerText = toolkit.truncate_text(avail_data.results[idx].last_name, props.max_len.last_name);
        render_detail_tab_btn(first_name_field.children[0], props.detail, avail_data.results[idx].id);
        render_detail_tab_btn(last_name_field.children[0],  props.detail, avail_data.results[idx].id);
        time_joined_field.innerText  = avail_data.results[idx].time_created.substr(0, props.max_len.date_joined);
        last_updated_field.innerText = avail_data.results[idx].last_updated.substr(0, props.max_len.last_updated);
        setup_canlogin_field(can_login_field, avail_data.results[idx]);
    } // end of loop
    for(idx += 2; idx < table_dom.children.length; idx++) {
        table_dom.children[idx].hidden = true;
    }
} // end of load_api_listusrprof_cb


function setup_canlogin_field(field, data){
    var elm_switch = field.querySelector("div[class*='custom-switch']");
    var elm_chkbox = field.querySelector("input[class=custom-control-input]");
    var elm_label  = field.querySelector("label[class=custom-control-label]");
    var unique_chkbox_id = "usr_login_switch_"+ data.id;
    elm_label.setAttribute('for', unique_chkbox_id);
    elm_chkbox.addEventListener('change', toggle_auth_activation);
    elm_chkbox.id = unique_chkbox_id;
    elm_chkbox.checked = data.active;
    elm_chkbox.dataset.user_id = data.id;
    elm_chkbox.dataset.first_name = data.first_name;
    elm_chkbox.dataset.last_name  = data.last_name;
    if(data.active) {
        if(data.auth) {
            elm_switch.classList.remove("custom-switch-on-warning");
            elm_switch.title = "active user, login account already created";
        } else {
            elm_switch.classList.add("custom-switch-on-warning");
            elm_switch.title = "active user, but hasn't created login account";
        }
    } else {
        elm_switch.title = "inactive user";
    }
}


function load_api_listroles_cb(avail_data, props) {
    if (!avail_data.results) {
        return;
    }
    var record_list = props.record_list;
    var table_dom = record_list.querySelector("table").children[0]; // auto generated tbody
    for(var idx = 0; idx < avail_data.results.length; idx++) {
        var record = _get_data_row_dom(table_dom, idx);
        var chkbox_field = record.querySelector("input[id=rid]");
        var name_field   = record.querySelector("td[id=name]");
        var permissions_field = record.querySelector("td[id=permissions]");
        chkbox_field.value  = avail_data.results[idx].id;
        chkbox_field.checked = false;
        name_field.children[0].innerText = toolkit.truncate_text(avail_data.results[idx].name, props.max_len.name);
        render_detail_tab_btn(name_field.children[0], props.detail, avail_data.results[idx].id);
        var perms = avail_data.results[idx].permissions.map((x) => {return "<button class='btn btn-sm btn-dark'>"+ x.name +"</button>";});
        permissions_field.innerHTML = perms.join(' ');
    }
    for(idx += 2; idx < table_dom.children.length; idx++) {
        table_dom.children[idx].hidden = true;
    }
} // end of load_api_listroles_cb


function load_api_listquota_cb(avail_data, props)
{
    if (!avail_data.results) {
        return;
    }
    var record_list = props.record_list;
    var table_dom = record_list.querySelector("table").children[0]; // auto generated tbody
    for(var idx = 0; idx < avail_data.results.length; idx++) {
        var record = _get_data_row_dom(table_dom, idx);
        var chkbox_field = record.querySelector("input[id=qid]");
        var name_field   = record.querySelector("td[id=label]");
        chkbox_field.value  = avail_data.results[idx].id;
        chkbox_field.checked = false;
        name_field.innerText = toolkit.truncate_text(avail_data.results[idx].label, props.max_len.label);
    }
    for(idx += 2; idx < table_dom.children.length; idx++) {
        table_dom.children[idx].hidden = true;
    }
} // end of load_api_listquota_cb


function load_api_activitylog_cb(avail_data, props)
{
    if (!avail_data.results) {
        return;
    }
    var record_list = props.record_list;
    var table_dom = record_list.querySelector("table").children[0]; // auto generated tbody
    for(var idx = 0; idx < avail_data.results.length; idx++) {
        var record = _get_data_row_dom(table_dom, idx);
        var action_field = record.querySelector("td[id=action]");
        var ipaddr_field = record.querySelector("td[id=ipaddr]");
        var timestamp_field = record.querySelector("td[id=timestamp]");
        action_field.innerHTML = activitylog.render_action(avail_data.results[idx], uri_dict);
        ipaddr_field.innerText = avail_data.results[idx].ipaddr;
        timestamp_field.innerText = avail_data.results[idx].timestamp;
    }
    for(idx += 2; idx < table_dom.children.length; idx++) {
        table_dom.children[idx].hidden = true;
    }
} // end of load_api_activitylog_cb


function load_api_loggerlevel_cb(avail_data, props)
{
    var caller = props.caller;
    var table_dom = caller.querySelector("table").children[0]; // auto generated tbody
    for(var idx = 0; idx < avail_data.length; idx++) {
        var record = _get_data_row_dom(table_dom, idx);
        var logger_name_field = record.querySelector("td[id=logger_name]");
        var edit_level_field  = record.querySelector("input[id=edit_level]");
        var apply_btn  = record.querySelector("button[id=apply]");
        var clean_btn  = record.querySelector("button[id=clean]");
        logger_name_field.innerHTML = avail_data[idx].name;
        edit_level_field.value  = avail_data[idx].level;
        apply_btn.dataset.base_url = props.api_base_url;
        apply_btn.dataset.logger_name = avail_data[idx].name;
        apply_btn.level_field = edit_level_field;
        apply_btn.removeEventListener('click', apply_loglevel_change);
        apply_btn.addEventListener('click', apply_loglevel_change);
        clean_btn.dataset.base_url = props.api_base_url;
        clean_btn.dataset.logger_name = avail_data[idx].name;
        clean_btn.removeEventListener('click', apply_loglevel_change);
        clean_btn.addEventListener('click', apply_loglevel_change);
    }
} // end of load_api_loggerlevel_cb


function default_detailtab_launch()
{ // must run as bound function
    for(var idx = 0; idx < this.apis.length; idx++) {
        var api = this.apis[idx];
        var tmp = api.url_template.split('/');
        tmp.pop(); // remove  empty string
        tmp.pop(); // remove <slug:pk>
        tmp.push(this.id);
        tmp.push("");
        var qualified_uri  = tmp.join('/');
        console.log("load data in the detail tab : "+ qualified_uri);
        api.url = qualified_uri;
        api.load_api_fn(api);
    }
    // simply for showing up the detail tab
    var evt = { target: {id: this.init_key,  tabs : global_tabs,}, };
    evt_switch_tab(evt);
} // end of default_detailtab_launch

function click_detailtab_btn(evt)
{
    evt.target._bound_fn_detailtab_launch();
}

function render_detail_tab_btn(target, detail, id)
{
    var obj = {id: id, ...detail};
    target._bound_fn_detailtab_launch  = default_detailtab_launch.bind(obj);
    target.removeEventListener('click', click_detailtab_btn);
    target.addEventListener('click', click_detailtab_btn);
} // end of render_detail_tab_btn



function load_api_detailrole_cb(avail_data, props) {
    //// console.log("start rendering with detail of the role ...");
    var accumulate_html = "";
    // TODO, the naming may not be good practice, rename the container
    var render_container = props.record_list;
    var rid_field   = render_container.querySelector("input[id=rid]:checked"); // should have only one checkbox
    var name_field  = render_container.querySelector("h3[id=name]"); // class="card-title"
    var perms_field = render_container.querySelector("div[id=permissions]");
    
    var _templates  = render_container.querySelector("div[id=templates]");
    var btn_perm    = _templates.querySelector("button[id=btn_perm]");

    rid_field.value = avail_data.id;
    name_field.innerHTML = avail_data.name;
    accumulate_html = "";
    for(var idx = 0; idx < avail_data.permissions.length; idx++) {
        btn_perm.innerHTML = avail_data.permissions[idx].name ;
        accumulate_html += btn_perm.outerHTML +" ";
    }
    perms_field.innerHTML = accumulate_html; // overwrite all previously-rendered child nodes
} // end of load_api_detailrole_cb


function load_api_grps_apply_role_cb(avail_data, props) {
    var accumulate_html = "";
    var num_items_per_page = props.query_params.page_size ;
    var render_container = props.record_list;
    var grps_field  = render_container.querySelector("div[id=groups]");
    
    var _templates  = render_container.querySelector("div[id=templates]");
    var btn_grpprof = _templates.querySelector("button[id=btn_grpprof]");
    
    accumulate_html = "";
    for(var idx = 0; idx < avail_data.results.length; idx++) {
        var item = avail_data.results[idx];
        btn_grpprof.innerHTML = item.name ;
        accumulate_html += btn_grpprof.outerHTML +" ";
    }
    if(avail_data.next) {
        accumulate_html += "<a href='#'>load next "+ num_items_per_page +" groups</a> ";
    }
    if(avail_data.previous) {
        grps_field.innerHTML += accumulate_html;
    } else {
        grps_field.innerHTML = accumulate_html;
    }
} // end of load_api_grps_apply_role_cb


function load_api_profs_apply_role_cb(avail_data, props) {
    var accumulate_html = "";
    var num_items_per_page = props.query_params.page_size ;
    var render_container = props.record_list;
    var profs_field = render_container.querySelector("div[id=profiles]");
    var _templates  = render_container.querySelector("div[id=templates]");
    var btn_grpprof = _templates.querySelector("button[id=btn_grpprof]");
    
    accumulate_html = "";
    for(var idx = 0; idx < avail_data.results.length; idx++) {
        var item = avail_data.results[idx];
        btn_grpprof.innerHTML = item.first_name +" "+ item.last_name;
        accumulate_html += btn_grpprof.outerHTML +" ";
    }
    if(avail_data.next) { // TODO, add event for loading next page
        accumulate_html += "<a href='#'>load next "+ num_items_per_page +" profiles</a> ";
    }
    if(avail_data.previous) {
        profs_field.innerHTML += accumulate_html;
    } else {
        profs_field.innerHTML = accumulate_html;
    }
}


function load_api_detail_usrgrp_cb(avail_data, props) {
    var accumulate_html = "";
    var idx = 0;
    var render_container = props.record_list;
    var name_field  = render_container.querySelector("h3[id=name]");
    var ancestors_field = render_container.querySelector("div[id=ancestors]");
    var ugid_field  = render_container.querySelector("input[id=ugid]:checked"); // should have only one checkbox
    var roles_field = render_container.querySelector("div[id=roles]");
    var quota_field = render_container.querySelector("div[id=quota]");

    var _templates  = render_container.querySelector("div[id=templates]");
    var btn_temp    = _templates.querySelector("button[id=btn_temp]");

    ugid_field.value = avail_data.id;
    name_field.innerHTML = avail_data.name;
    var asc_grp_props = {max_len: {hierarchy: 1000},
        detail: {apis: get_detail_usrgrp_api(global_tabs['detail_usr_grp'].dom),  init_key: 'detail_usr_grp', },
    }; // rebuild property for another detial tab ...
    render_hierarchy_text(ancestors_field, avail_data.ancestors, asc_grp_props);
    accumulate_html = "";
    for(var idx = 0; idx < avail_data.roles.length; idx++) {
        btn_temp.innerHTML = avail_data.roles[idx].name ;
        accumulate_html += btn_temp.outerHTML +" ";
    }
    roles_field.innerHTML = accumulate_html;

    simple_render_dom_rows(
        {dom:quota_field, row_id:'row_header', dst_id:'row_body', data:avail_data.quota,
        col_id_list:[
            {id:'maxnum',           access_fn:function(x){ return x.maxnum; }},
            {id:'usage_type_label', access_fn:function(x){ return x.usage_type.label; }},
        ], 
    });
} // end of load_api_detail_usrgrp_cb


function load_api_profs_apply_grp_cb(avail_data, props) {
    var accumulate_html = "";
    var idx = 0;
    var render_container = props.record_list;
    var num_items_per_page = props.query_params.page_size ;
    var profs_field = render_container.querySelector("div[id=profiles]");
    
    var _templates  = render_container.querySelector("div[id=templates]");
    var btn_temp    = _templates.querySelector("button[id=btn_temp]");
    
    accumulate_html = "";
    for(var idx = 0; idx < avail_data.results.length; idx++) {
        var item = avail_data.results[idx];
        btn_temp.innerHTML = item.first_name +" "+ item.last_name;
        accumulate_html += btn_temp.outerHTML +" ";
    }
    if(avail_data.next) { // TODO, add event for loading next page
        accumulate_html += "<a href='#'>load next "+ num_items_per_page +" profiles</a> ";
    }
    if(avail_data.previous) {
        profs_field.innerHTML += accumulate_html;
    } else {
        profs_field.innerHTML = accumulate_html;
    }
} // end of load_api_profs_apply_grp_cb


function load_api_detail_usrprof_cb(avail_data, props) {
    var accumulate_html = "";
    var idx = 0;
    var render_container = props.record_list;
    var first_name_field  = render_container.querySelector("h3[id=first_name]");
    var last_name_field   = render_container.querySelector("h3[id=last_name]");
    var upid_field    = render_container.querySelector("input[id=upid]:checked"); // should have only one checkbox
    var groups_field  = render_container.querySelector("div[id=groups]");
    var roles_field   = render_container.querySelector("div[id=roles]");
    var quota_field   = render_container.querySelector("div[id=quota]");
    var emails_field  = render_container.querySelector("div[id=emails]");
    var phones_field  = render_container.querySelector("div[id=phones]");
    var geoloc_field  = render_container.querySelector("div[id=geolocations]");

    var _templates  = render_container.querySelector("div[id=templates]");
    var btn_temp    = _templates.querySelector("button[id=btn_temp]");

    upid_field.value = avail_data.id;
    first_name_field.innerHTML = avail_data.first_name;
    last_name_field.innerHTML  = avail_data.last_name;
    
    accumulate_html = "";
    for(var idx = 0; idx < avail_data.groups.length; idx++) {
        btn_temp.innerHTML = avail_data.groups[idx].name ;
        accumulate_html += btn_temp.outerHTML +" ";
    }
    groups_field.innerHTML = accumulate_html;
    
    accumulate_html = "";
    for(var idx = 0; idx < avail_data.roles.length; idx++) {
        btn_temp.innerHTML = avail_data.roles[idx].name ;
        accumulate_html += btn_temp.outerHTML +" ";
    }
    roles_field.innerHTML = accumulate_html;

    simple_render_dom_rows(
        {dom:quota_field, row_id:'row_header', dst_id:'row_body', data:avail_data.quota,
        col_id_list:[
            {id:'maxnum',           access_fn:function(x){ return x.maxnum; }},
            {id:'usage_type_label', access_fn:function(x){ return x.usage_type.label; }},
        ], 
    });
    simple_render_dom_rows(
        {dom:emails_field, row_id:'row_header', dst_id:'row_body', data:avail_data.emails,
        col_id_list:[
            {id:'addr',  access_fn:function(x){ return x.addr; }},
        ], 
    });
    simple_render_dom_rows(
        {dom: phones_field, row_id:'row_header', dst_id:'row_body', data:avail_data.phones,
        col_id_list:[
            {id:'country_code',  access_fn:function(x){ return x.country_code; }},
            {id:'line_number',   access_fn:function(x){ return x.line_number; }},
        ], 
    });
    simple_render_dom_rows(
        {dom: geoloc_field, row_id:'row_header', dst_id:'row_body', data:avail_data.locations,
        col_id_list:[
            {id:'country',   access_fn:function(x){ return x.country; }},
            {id:'province',  access_fn:function(x){ return x.province; }},
            {id:'locality',  access_fn:function(x){ return x.locality; }},
            {id:'street',  access_fn:function(x){ return x.street; }},
            {id:'detail',  access_fn:function(x){ return x.detail; }},
            {id:'floor',   access_fn:function(x){ return x.floor; }},
            {id:'description',  access_fn:function(x){ return x.description; }},
        ], 
    });
} // end of load_api_detail_usrprof_cb


function simple_render_dom_rows(kwargs) {
    var dom = kwargs.dom;
    var data = kwargs.data;
    var row_id = kwargs.row_id;
    var col_id_list = kwargs.col_id_list;
    var dst_id = kwargs.dst_id;

    var accumulate_html = "";
    var row  = dom.querySelector("div[id="+ row_id +"]");
    col_id_list.map((x) => {
        x.obj = row.querySelector("div[id="+ x.id +"]");
        x.bak_value = x.obj.innerText;
        return x;
    });
    for(var idx = 0; idx < data.length; idx++) {
        var item = data[idx];
        col_id_list.map((x) => {
            x.obj.innerText = x.access_fn(item);
            return x;
        });
        accumulate_html += row.outerHTML;
    }
    var dst = dom.querySelector("div[id="+ dst_id +"]");
    dst.innerHTML = accumulate_html;
    col_id_list.map((x) => {
        x.obj.innerText = x.bak_value;
        return x;
    });
} // end of simple_render_dom_rows



function api_delete_items_cb(avail_data, props) {
    var caller = props.caller;
    // note, `204`:permenant;y deleted, `202`:soft-deleted, not accessiable for client
    if (props.http_resp_status == 204 || props.http_resp_status == 202) {
        // refresh the page if response is 204 (deleted successfully)
        //// perform_api_call(caller.api);
        caller.api.load_api_fn(caller.api);
    }
}

// TODO, re-factor
function prompt_api_resp_msg(avail_data, props) {
    var caller = props.caller;
    var base_url = caller.dataset.base_url;
    var prompt_header_msg = null;
    var verb_msg = null;
    var render_msg = null;
    var items = null;
    var alert_width = 500;
    var alert_icon = 'success';
    var logout_flag = false;

    if (props.http_resp_status == 204) {
        verb_msg = "permenantly removed";
    } else if (props.http_resp_status == 202) {
        verb_msg = "removed<br>If it was a mistake, click `undelete` to rollback";
        alert_width = 800;
    }
    switch (Math.floor(props.http_resp_status/100)) {
        case 4:
            alert_icon = 'error';
            verb_msg = avail_data.non_field_errors;
            break;
        case 5:
            alert_icon = 'warning';
            verb_msg = avail_data.non_field_errors;
            break;
    }
    switch (base_url)
    {
        case uri_dict['UserGroupsAPIView'][0]:
            prompt_header_msg = "User group(s)";
            break;
        case "/usermgt/usergrps/undel": // TODO, fix display problem
            prompt_header_msg = "User group(s)";
            verb_msg = ": "+ avail_data.message;
            if (props.http_resp_status == 200) {
                var grps  = avail_data.params.recovered;
                verb_msg += " undeleted groups : "+ grps.map((x) => {
                    return "<a href='/usermgt/usergrp/"+ x.id +"'>"+ x.name +"</a>";
                }).join(', ');
            }
            break;
        case uri_dict['UserProfileAPIView'][0]:
            prompt_header_msg = "User Profile(s)";
            break;
        case "/usermgt/users/undel":
            prompt_header_msg = "User Profile(s)";
            verb_msg = ": "+ avail_data.message;
            if (props.http_resp_status == 200) {
                var grps  = avail_data.params.recovered;
                verb_msg += " undeleted users : "+ grps.map((x) => {
                    return "<a href='/usermgt/user/"+ x.id +"'>"+ x.first_name +" "+ x.last_name +"</a>";
                }).join(', ');
            }
            break;
        case uri_dict['UserActivationView']:
            // TODO, frontend may check status of mailing task at later time
            prompt_header_msg = "User";
            verb_msg = ": Activation URL is sent to users' e-mail inbox";
            break;
        case uri_dict['UserDeactivationView']:
            prompt_header_msg = "User";
            verb_msg = " deactivated";
            break;
        case uri_dict['AuthRoleAPIView'][0]:
            prompt_header_msg = "Authorization role(s)";
            break;
        case uri_dict['QuotaUsageTypeAPIView']:
            prompt_header_msg = "Quota usage type(s)";
            break;
        case uri_dict['AuthUsernameEditAPIView']:
            prompt_header_msg = "Account username";
            if (props.http_resp_status == 200) {
                verb_msg = "updated successfully, use new username next time when you log in";
            }
            break;
        case uri_dict['AuthPasswdEditAPIView']:
            prompt_header_msg = "Account password";
            if (props.http_resp_status == 200) {
                verb_msg = "updated successfully, current session expired automatically, please log in again with the new password.";
            }
            break;
        case uri_dict['LogoutView']:
            prompt_header_msg = "User";
            verb_msg = " logged out";
            logout_flag = true;
            break;
        case uri_dict['DynamicLoglevelAPIView']:
            prompt_header_msg = "logger level";
            if(props.req_opt.method == 'PUT') {
                verb_msg = " changed";
            } else  if(props.req_opt.method == 'DELETE'){
                verb_msg = " reset to default value";
            }
            break;
        default:
            console.log("unknown API call: "+ base_url);
            break;
    }
    if(prompt_header_msg) {
        render_msg = prompt_header_msg;
    }
    if(verb_msg) {
        render_msg += " "+ verb_msg;
    }
    if(render_msg) { // show up status prompt html element
        alert_prompt_ctrl.fire({
            icon: alert_icon,  title: 'Done',
            html: render_msg, timer: 5500,
            width: alert_width
        });
        if(logout_flag) {
            setTimeout(
                function(){
                    window.location.href = uri_dict['LoginView'];
                }, 3000
            );
        }
    } else {
        console.log("no message to prompt");
    }
} // end of prompt_api_resp_msg


function perform_api_call(props)
{
    var api_handler = new toolkit.APIconsumer(
        {
            api_base_url: props.url, req_opt: props.req_opt, query_params: props.query_params,
            finish_cbs: props.callbacks, record_list: props.record_list, paging_panel: props.paging_panel,
            caller:props.caller, max_len:props.max_len,
            //// detail_url: props.detail_url,
            detail: props.detail,
        }
    );
    api_handler.start( props.query_params );
}

function _get_edit_btns(container) {
    var names = ['add_items', 'edit_items', 'del_items', 'undel_item', 'back_to_list'];
    var query_string = names.map((x) => "a[id="+ x +"]").join(',');
    return Array.from(container.querySelectorAll(query_string));
}


function init_toolbar_list_usrgrp(new_dom, api) {
    var doms = _get_edit_btns(new_dom);
    var btn_data = {};
    btn_data['add_items']  = {title:'add new groups', href:uri_dict['UserGroupsAddHTMLView'], };
    btn_data['edit_items'] = {title:'edit chosen groups',   onclick: run_action_bulk, api: api,
            dataset:{cls_name:'ugid', command:'edit', base_url:uri_dict['UserGroupsUpdateHTMLView'],
            extra_query_params: "parent_only=yes&fields="+ encodeURIComponent( global_tabs['detail_usr_grp'].fetch_fields.join(',') )
            },};
    btn_data['del_items']  = {title:'delete chosen groups', onclick: run_action_bulk, api: api,
            dataset:{cls_name:'ugid', command:'del',  base_url:uri_dict['UserGroupsAPIView'][0],}, };
    btn_data['undel_item'] = {title:'undo last deleted groups', onclick: run_recovery, api:api, hidden:false,
            dataset:{command:'recovery', time_start:'', base_url:uri_dict['UserGroupsAPIView'][0],},}
    // time_start 2020-9-5-0-59
    toolkit.update_doms_props(doms, btn_data);
    init_search_bar(new_dom, api, {textfield:'search_keywords', button:'search_submit'});
} // end of init_toolbar_list_usrgrp


function init_toolbar_list_usrprof(new_dom, api)
{
    var doms = _get_edit_btns(new_dom);
    var btn_data = {};
    
    btn_data['add_items']  = {title:'add users', href: uri_dict['UserProfileAddHTMLView'], };
    btn_data['edit_items'] = { title:'edit chosen users',  onclick: run_action_bulk, api: api,
        dataset:{ cls_name:'upid', command:'edit', base_url: uri_dict['UserProfileUpdateHTMLView'],
            extra_query_params: "fields="+ encodeURIComponent( global_tabs['detail_usr_prof'].fetch_fields.join(',') )
        },
    };
    btn_data['del_items']  = {title:'delete chosen users', onclick: run_action_bulk,  api: api,
            dataset:{cls_name:'upid', command:'del',  base_url: uri_dict['UserProfileAPIView'][0],}, };
    btn_data['undel_item'] = {title:'undo last deleted profiles', onclick: run_recovery, api:api, hidden:false,
            dataset:{command:'recovery', time_start:'', base_url: uri_dict['UserProfileAPIView'][0],},}
    toolkit.update_doms_props(doms, btn_data);
    init_search_bar(new_dom, api, {textfield:'search_keywords', button:'search_submit'});
}


function init_toolbar_list_authrole(new_dom, api)
{
    var doms = _get_edit_btns(new_dom);
    var btn_data = {};
    btn_data['add_items']  = {title:'add roles', href: uri_dict['AuthRoleAddHTMLView'], };
    btn_data['edit_items'] = {title:'edit chosen roles',   onclick: run_action_bulk, api: api,
                dataset:{cls_name:'rid', command:'edit', base_url: uri_dict['AuthRoleUpdateHTMLView'],
                extra_query_params: "fields="+ api.query_params.fields}, };
    btn_data['del_items']  = {title:'delete chosen roles', onclick: run_action_bulk, api:api,
            dataset:{cls_name:'rid', command:'del',  base_url: uri_dict['AuthRoleAPIView'][0],}, };
    toolkit.update_doms_props(doms, btn_data);
    init_search_bar(new_dom, api, {textfield:'search_keywords', button:'search_submit'});
}


function init_toolbar_list_quota(new_dom, api)
{
    var doms = _get_edit_btns(new_dom);
    var btn_data = {};
    var fetch_fields = [api.query_params.fields, 'material', 'appname'];
    fetch_fields = encodeURIComponent(fetch_fields.join(","));
    btn_data['add_items']  = {title:'add quota usage type', href: uri_dict['QuotaUsageTypeAddHTMLView'], };
    btn_data['edit_items'] = {title:'edit chosen quota usage type',   onclick: run_action_bulk, api:api,
                dataset:{cls_name:'qid', command:'edit', base_url: uri_dict['QuotaUsageTypeUpdateHTMLView'],
                extra_query_params: "fields="+ fetch_fields},};
    btn_data['del_items']  = {title:'delete chosen quota usage type', onclick: run_action_bulk, api:api,
            dataset:{cls_name:'qid', command:'del',  base_url: uri_dict['QuotaUsageTypeAPIView'],}, };
    toolkit.update_doms_props(doms, btn_data);
    init_search_bar(new_dom, api, {textfield:'search_keywords', button:'search_submit'});
}


function init_toolbar_detail_usrgrp(new_dom, api)
{
    var doms = _get_edit_btns( new_dom );
    var btn_data = {};
    btn_data['add_items'] = {hidden:true};
    btn_data['del_items'] = {hidden:true};
    btn_data['edit_items'] = {title:'edit this group',   onclick: run_action_bulk, api: api,
            dataset:{cls_name:'ugid', command:'edit', base_url: uri_dict['UserGroupsUpdateHTMLView'],
            extra_query_params: "parent_only=yes&fields="+ api.query_params.fields}, };
    btn_data['back_to_list']  = {title:'go back to list of groups', onclick: evt_switch_tab, hidden: false,
            tabs: global_tabs, dataset:{substitute_id: 'list_grps',}, };
    toolkit.update_doms_props(doms, btn_data);
    // implicitly turn off the search bar in detail tab
    init_search_bar(new_dom, null, {textfield:'search_keywords', button:'search_submit'});
}


function init_toolbar_detail_usrprof(new_dom, api)
{
    var doms = _get_edit_btns( new_dom );
    var btn_data = {};
    btn_data['add_items'] = {hidden:true};
    btn_data['del_items'] = {hidden:true};
    btn_data['edit_items'] = {title:'edit this profile',   onclick: run_action_bulk, api: api,
            dataset:{cls_name:'upid', command:'edit', base_url: uri_dict['UserProfileUpdateHTMLView'],
            extra_query_params: "fields="+ api.query_params.fields}, };
    btn_data['back_to_list']  = {title:'go back to list of profiles', onclick: evt_switch_tab, hidden: false,
            tabs: global_tabs, dataset:{substitute_id: 'list_usrs',}, };
    toolkit.update_doms_props(doms, btn_data);
    // implicitly turn off the search bar in detail tab
    init_search_bar(new_dom, null, {textfield:'search_keywords', button:'search_submit'});
}


function init_toolbar_detail_authrole(new_dom, api)
{
    var doms = _get_edit_btns( new_dom );
    var btn_data = {};
    btn_data['add_items'] = {hidden:true};
    btn_data['del_items'] = {hidden:true};
    btn_data['edit_items'] = {title:'edit this role',   onclick: run_action_bulk, api: api,
            dataset:{cls_name:'rid', command:'edit', base_url: uri_dict['AuthRoleUpdateHTMLView'],
            extra_query_params: "fields="+ api.query_params.fields}, };
    btn_data['back_to_list']  = {title:'go back to list of roles', onclick: evt_switch_tab, hidden: false,
            tabs: global_tabs, dataset:{substitute_id: 'list_auth_roles',}, };
    toolkit.update_doms_props(doms, btn_data);
    // implicitly turn off the search bar in detail tab
    init_search_bar(new_dom, null, {textfield:'search_keywords', button:'search_submit'});
}


function init_common_panels(tab, templates) {
    var skipped_props = ['dom', 'init_fn', 'detail', 'fetch_fields']
    for(var key in tab) {
        if(skipped_props.indexOf(key) >= 0) {
            continue;
        }
        var record_list = tab[key].dom.querySelector("div[id=record_list]"); // record list is always put at bottom
        var api = tab[key].api;
        api.record_list = record_list;
        init_order_btns(api);
        for(var idx = 0; idx < templates.children.length; idx++) {
            var panel_name = templates.children[idx].id;
            var custom_fn  = tab[key].custom_fn[panel_name];
            if(custom_fn != undefined) {
                var copied = templates.children[idx].cloneNode(true);
                custom_fn(copied, api);
                tab[key].dom.insertBefore(copied, record_list);
            }
        }
        if(api && api.load_api_fn) {
            api.load_api_fn(api);
        }
    }
} // end of init_common_panels


function check_if_inited(dom)
{
    var out = false;
    if(dom.inited == undefined) {
        dom.inited = true;
    } else {
        console.log(dom.dataset.tabname +" already initialized");
        out = true;
    }
    return out;
}


function get_detail_usrgrp_api(detail_render_container)
{
    var detail_api = {
        req_opt: common_req_opt, url:'xxx/yyy', url_template: uri_dict['UserGroupsAPIView'][1],
        load_api_fn: perform_api_call,  callbacks:[load_api_detail_usrgrp_cb], record_list: detail_render_container,
        query_params:{fields: global_tabs['detail_usr_grp'].fetch_fields.join(','), },
    };
    var grp_to_profs_api = {
        req_opt: common_req_opt, url:'xxx/yyy', url_template: uri_dict['AppliedGroupReadAPIView'],
        load_api_fn: perform_api_call,  callbacks:[load_api_profs_apply_grp_cb],
        query_params:{fields:'id,first_name,last_name', page_size:7}, record_list: detail_render_container,
    };
    return [detail_api, grp_to_profs_api];
}


function _init_listusrgrp_tab(tab, detail_render_container)
{
    var usr_grp  = {};
    usr_grp.dom  = tab.dom.querySelector('div[id=usr_grp]');
    usr_grp.custom_fn  = {tool_bar:init_toolbar_list_usrgrp, };

    usr_grp.api  = {
        req_opt: common_req_opt, url: uri_dict['UserGroupsAPIView'][0],
        detail: {apis: get_detail_usrgrp_api(detail_render_container),  init_key: null, },
        load_api_fn:perform_api_call,  callbacks:[load_api_listusrgrp_cb,],
        query_params:{page_size: -1, page:1, ordering:'id',
            fields: global_tabs['list_grps'].fetch_fields.join(',')
        },
        avail_order_fields:['name', 'usr_cnt'], max_len: null,  default_page_size: -1,
    };
    tab.usr_grp  = usr_grp ;
}

function _init_listusrprof_tab(tab, detail_render_container)
{
    var usr_prof = {};
    usr_prof.dom = tab.dom.querySelector('div[id=usr_prof]');
    usr_prof.custom_fn = {tool_bar:init_toolbar_list_usrprof, };
    var detail_api = {
        req_opt: common_req_opt, url:'xxx/yyy', url_template: uri_dict['UserProfileAPIView'][1],
        load_api_fn: perform_api_call,  callbacks:[load_api_detail_usrprof_cb], record_list: detail_render_container,
        query_params:{fields: global_tabs['detail_usr_prof'].fetch_fields.join(','), },
    };

    usr_prof.api = {
        req_opt: common_req_opt, url:uri_dict['UserProfileAPIView'][0],
        detail: {apis: [detail_api],  init_key: null,},
        load_api_fn:perform_api_call, callbacks:[load_api_listusrprof_cb,],
        query_params:{
            page_size: -1, page:1, ordering:'-time_created',
            fields: global_tabs['list_usrs'].fetch_fields.join(',')
        },
        avail_order_fields:['first_name', 'last_name', 'date_joined', 'last_updated'],
        max_len: null, default_page_size: -1,
    };
    tab.usr_prof = usr_prof;
}


function init_summary_tab(tab, templates)
{
    if(check_if_inited(tab.dom)) { return; }
    _init_listusrgrp_tab(tab, tab.detail.dom_dict[tab.detail.init_key[0]]);
    tab.usr_grp.api.max_len = {name:15, hierarchy:32};
    tab.usr_grp.api.query_params.page_size = 5;
    tab.usr_grp.api.default_page_size = 5;
    tab.usr_grp.api.detail.init_key = tab.detail.init_key[0];

    _init_listusrprof_tab(tab, tab.detail.dom_dict[ tab.detail.init_key[1] ]);
    tab.usr_prof.api.max_len = {first_name:10, last_name:10, date_joined:10, last_updated:19 };
    tab.usr_prof.api.query_params.page_size = 5;
    tab.usr_prof.api.default_page_size = 5;
    tab.usr_prof.api.detail.init_key = tab.detail.init_key[1];

    init_common_panels(tab, templates);
} // end of init_summary_tab


function init_listusrgrp_tab(tab, templates)
{
    if(check_if_inited(tab.dom)) { return; }
    const num_records = 17;
    _init_listusrgrp_tab(tab, tab.detail.dom_dict[tab.detail.init_key[0]]);
    tab.usr_grp.custom_fn['paging_panel'] = init_paging_panel;
    tab.usr_grp.api.max_len = {name:20, hierarchy:75};
    tab.usr_grp.api.query_params.page_size = num_records;
    tab.usr_grp.api.default_page_size = num_records;
    tab.usr_grp.api.callbacks.push(update_pagination_cb);
    tab.usr_grp.api.detail.init_key = tab.detail.init_key[0];

    init_common_panels(tab, templates);
} // end of init_listusrgrp_tab


function init_listusrprof_tab(tab, templates)
{
    if(check_if_inited(tab.dom)) { return; }
    const num_records = 10;
    _init_listusrprof_tab(tab, tab.detail.dom_dict[ tab.detail.init_key[0] ]);
    tab.usr_prof.custom_fn['paging_panel'] = init_paging_panel;
    tab.usr_prof.api.max_len = {first_name:10, last_name:10, date_joined:19, last_updated:19 };
    tab.usr_prof.api.query_params.page_size = num_records;
    tab.usr_prof.api.default_page_size = num_records;
    tab.usr_prof.api.callbacks.push(update_pagination_cb);
    tab.usr_prof.api.detail.init_key = tab.detail.init_key[0];

    init_common_panels(tab, templates);
} // end of init_listusrprof_tab


function init_listrole_tab(tab, templates)
{
    const num_records = 15;
    var auth_role = {};
    if(check_if_inited(tab.dom)) { return; }
    auth_role.dom = tab.dom.querySelector('div[id=auth_role]');
    auth_role.custom_fn = {tool_bar:init_toolbar_list_authrole, paging_panel:init_paging_panel };
    var detail_render_container = tab.detail.dom_dict[ tab.detail.init_key[0] ];
    var detail_role_api = {
        req_opt: common_req_opt, url:'xxx/yyy', url_template: uri_dict['AuthRoleAPIView'][1],
        load_api_fn: perform_api_call,  callbacks:[load_api_detailrole_cb],
        query_params:{fields: tab.fetch_fields.join(',')}, record_list: detail_render_container,
    };
    var role_to_grps_api = {
        req_opt: common_req_opt, url:'xxx/yyy', url_template: uri_dict['AppliedRoleReadAPIView'],
        load_api_fn: perform_api_call,  callbacks:[load_api_grps_apply_role_cb],
        query_params:{fields:'id,name', type:'groups', page_size:6}, record_list: detail_render_container,
    };
    var role_to_profs_api = {
        req_opt: common_req_opt, url:'xxx/yyy', url_template: uri_dict['AppliedRoleReadAPIView'],
        load_api_fn: perform_api_call,  callbacks:[load_api_profs_apply_role_cb],
        query_params:{fields:'id,first_name,last_name', type:'profiles', page_size:6}, record_list: detail_render_container,
    };
    auth_role.api = {
        req_opt: common_req_opt, url: uri_dict['AuthRoleAPIView'][0],
        detail: {apis: [detail_role_api, role_to_grps_api, role_to_profs_api], init_key: tab.detail.init_key[0],},
        load_api_fn:perform_api_call,  callbacks:[load_api_listroles_cb, update_pagination_cb],
        query_params:{page_size:num_records, page:1, ordering:'name', fields: tab.fetch_fields.join(',')},
        avail_order_fields:['name',],  max_len: {name:20,},  default_page_size: num_records,
    };
    tab.auth_role = auth_role;
    init_common_panels(tab, templates);
} // end of init_listrole_tab


function init_listquota_tab(tab, templates)
{
    const num_records = 15;
    var content_wrapper = {};
    if(check_if_inited(tab.dom)) { return; }
    content_wrapper.dom = tab.dom.querySelector('div[id=content-wrapper]');
    content_wrapper.custom_fn = {tool_bar:init_toolbar_list_quota, paging_panel:init_paging_panel };
    content_wrapper.api = {
        req_opt: common_req_opt, url: uri_dict['QuotaUsageTypeAPIView'], load_api_fn:perform_api_call,
        query_params:{page_size:num_records, page:1, ordering:'label', fields:'id,label'},
        callbacks:[load_api_listquota_cb, update_pagination_cb], default_page_size: num_records,
        avail_order_fields:['label',],  max_len: {label:50,},
    };
    tab.content_wrapper = content_wrapper;
    init_common_panels(tab, templates);
}

function init_activity_log_tab(tab, templates)
{
    const num_records = 12;
    var content_wrapper = {};
    if(check_if_inited(tab.dom)) { return; }
    content_wrapper.dom = tab.dom.querySelector('div[id=content-wrapper]');
    content_wrapper.custom_fn = {paging_panel:init_paging_panel };
    content_wrapper.api = {
        req_opt: common_req_opt, url: uri_dict['UserActionHistoryAPIReadView'], load_api_fn:perform_api_call,
        query_params:{page_size:num_records, page:1, ordering:'timestamp',
           //date__gte:'2021-01-10T00:00:00.002', date__lt:'2021-01-10T14:08:00.001'
        },
        callbacks:[load_api_activitylog_cb, update_pagination_cb], default_page_size: num_records,
        avail_order_fields:['action', 'ipaddr', 'timestamp'],  max_len: {action:70,},
    };
    tab.content_wrapper = content_wrapper;
    init_common_panels(tab, templates);
}


function _init_detail_common_tab(tab, templates, custom_fn)
{
    if(check_if_inited(tab.dom)) { return; }
    var content_wrapper = {};
    content_wrapper.dom = tab.dom.querySelector('div[id=content-wrapper]');
    content_wrapper.custom_fn = custom_fn;
    content_wrapper.api = { query_params:{fields: tab.fetch_fields.join(',')}, };
    tab.content_wrapper = content_wrapper;
    init_common_panels(tab, templates);
}

function init_detailusrgrp_tab(tab, templates)
{
    var custom_fn = {tool_bar: init_toolbar_detail_usrgrp, };
    _init_detail_common_tab(tab, templates, custom_fn);
}

function init_detailusrprof_tab(tab, templates)
{
    var custom_fn = {tool_bar: init_toolbar_detail_usrprof, };
    _init_detail_common_tab(tab, templates, custom_fn);
}

function init_detailrole_tab(tab, templates)
{
    var custom_fn = {tool_bar: init_toolbar_detail_authrole, };
    _init_detail_common_tab(tab, templates, custom_fn);
}


function _init_settings_common_subforms(card, init_data)
{
    var form_layout = null;
    var submit_btn  = null;
    form_layout = card.querySelector('div[id=form_layout]');
    form_layout.dataset.is_single_form = true;
    
    submit_btn  = card.querySelector('button[id=submit_button]');
    submit_btn.forms = form_layout;
    submit_btn.dataset.api_mthd = init_data.api_mthd;
    submit_btn.dataset.api_base_url = init_data.submit_url;
    submit_btn.dataset.base_url     = init_data.submit_url;
    submit_btn.finish_callbacks = [prompt_api_resp_msg,];
    submit_btn.query_params = {};
    submit_btn.addEventListener('click', toolkit.on_submit_forms);
    return form_layout;
}


function init_settings_edit_uname_card(tab)
{
    var card = tab.dom.querySelector('div[id=edit_username]');
    var form_layout = _init_settings_common_subforms(card, tab.init_data.edit_uname);
    var props = login_account.get_default_form_props(form_layout);
    props.btns.addform.enable = false;
    props.enable_close_btn = false;
    props.form['password'].disabled = true;
    props.form['password2'].disabled = true;
    props.form['old_passwd'].disabled = true;
    login_account.append_new_form(form_layout, [props]);
} // end of init_settings_edit_uname_card


function init_settings_edit_passwd_card(tab)
{
    var card = tab.dom.querySelector('div[id=edit_password]');
    var form_layout = _init_settings_common_subforms(card, tab.init_data.edit_passwd);
    var props = login_account.get_default_form_props(form_layout);
    props.btns.addform.enable = false;
    props.enable_close_btn = false;
    props.form['username'].disabled = true;
    props.form['old_uname'].disabled = true;
    login_account.append_new_form(form_layout, [props]);
} // end of init_settings_edit_passwd_card


function init_settings_dynamic_loglevel(tab)
{
    var card = tab.dom.querySelector('div[id=dynamic_loglevel_editor]');
    var api = {
        req_opt: common_req_opt, url: uri_dict['DynamicLoglevelAPIView'],
        query_params:{},  callbacks:[load_api_loggerlevel_cb], caller: card,
    };
    perform_api_call(api);
}


function init_settings_tab(tab, templates)
{
    if(check_if_inited(tab.dom)) { return; }
    init_settings_edit_uname_card(tab);
    init_settings_edit_passwd_card(tab);
    init_settings_dynamic_loglevel(tab);
} // end of init_settings_tab


function init_tab_btn(tabs) {
    var btn_data = {};
    var tab_btns = document.getElementById('tab_btns');
    var doms = Array.from(tab_btns.querySelectorAll("a"));
    for(var idx = 0; idx < doms.length; idx++) {
        var _id = doms[idx].id;
        btn_data[_id] = {onclick: [evt_switch_tab, evt_change_tabbtn_bg], tabs:tabs, tab_btns:doms};
        ////btn_data[_id] = {onclick: evt_switch_tab, tabs:tabs, tab_btns:doms};
    }
    toolkit.update_doms_props(doms, btn_data);
}


function on_page_load()
{
    const init_data = JSON.parse(document.getElementById('form_init_data').textContent);
    uri_dict = init_data.uri_dict;
    
    alert_prompt_ctrl = Swal.mixin({
        toast: true,
        position: 'top-end',
        showConfirmButton: true,
    });
    global_templates = document.getElementById('templates');
    var detail_tabs = null;
    detail_tabs = {
                detail_usr_prof  : document.querySelector('div[id=tab][data-tabname=detail_usr_prof]'),
                detail_usr_grp   : document.querySelector('div[id=tab][data-tabname=detail_usr_grp]'),
                detail_auth_roles: document.querySelector('div[id=tab][data-tabname=detail_auth_roles]'),
            };
    global_tabs = {
        summary:{
            init_fn: init_summary_tab,
            dom: document.querySelector('div[id=tab][data-tabname=summary]'),
            detail: {dom_dict: detail_tabs, init_key:['detail_usr_grp', 'detail_usr_prof'], },
        },
        list_grps:{
            init_fn: init_listusrgrp_tab,
            dom: document.querySelector('div[id=tab][data-tabname=list_grps]'),
            detail: {dom_dict: detail_tabs, init_key:['detail_usr_grp'], },
            fetch_fields: ['id','name','usr_cnt','ancestors','ancestor','depth'],
        },
        list_usrs:{
            init_fn: init_listusrprof_tab,
            dom: document.querySelector('div[id=tab][data-tabname=list_usrs]'),
            detail: {dom_dict: detail_tabs, init_key:['detail_usr_prof'], },
            fetch_fields: ['id','first_name','last_name','active', 'time_created', 'last_updated', 'auth'],
        },
        list_auth_roles:{
            init_fn: init_listrole_tab,
            dom: document.querySelector('div[id=tab][data-tabname=list_auth_roles]'),
            detail: {dom_dict: detail_tabs, init_key:['detail_auth_roles'], },
            fetch_fields: ['id','name','permissions'],
        },
        list_quotas: {
            init_fn: init_listquota_tab,
            dom: document.querySelector('div[id=tab][data-tabname=list_quotas]'),
            fetch_fields: ['id','label'],
        },
        detail_auth_roles:{
            init_fn: init_detailrole_tab,
            dom: detail_tabs['detail_auth_roles'],
            fetch_fields: ['id','name','permissions'],
        },
        detail_usr_prof: {
            init_fn: init_detailusrprof_tab,
            dom: detail_tabs['detail_usr_prof'],
            fetch_fields: ['id','first_name','last_name','groups','roles','quota','maxnum','usage_type',
                    'label', 'phones', 'phone', 'country_code', 'line_number','emails', 'email', 'addr',
                    'locations','address', 'country', 'province', 'locality', 'street', 'detail',
                    'floor', 'description',],
        },
        detail_usr_grp: {
            init_fn: init_detailusrgrp_tab,
            dom: detail_tabs['detail_usr_grp'],
            fetch_fields: ['id','name','ancestors','depth','ancestor','roles','quota','maxnum',
                'usage_type','label'],
        },
        activity_log: {
            init_fn: init_activity_log_tab,
            dom: document.querySelector('div[id=tab][data-tabname=activity_log]'),
            fetch_fields: ['action', 'ipaddr', 'timestamp'], // no need to load id since these are read-only
        },
        settings: {
            init_fn: init_settings_tab,
            init_data: {
                edit_uname: {api_mthd:'PATCH', submit_url: uri_dict['AuthUsernameEditAPIView'],},
                edit_passwd:{api_mthd:'PATCH', submit_url: uri_dict['AuthPasswdEditAPIView'],},
            },
            dom: document.querySelector('div[id=tab][data-tabname=settings]'),
        },
    };
    init_tab_btn(global_tabs);
    global_tabs.summary.init_fn(global_tabs.summary, global_templates);
    // 
    var btn_logout = document.getElementById('btn_logout');
    btn_logout.addEventListener('click', perform_logout);
    btn_logout.dataset.base_url = uri_dict['LogoutView'];
} // end of on_page_load

window.onload = on_page_load();

