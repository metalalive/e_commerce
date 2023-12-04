
export function render_action(item, uri_dict)
{
    var extra_info_fields = {
        'create'   : _render_delete,
        'update'   : _render_delete,
        'delete'   : _render_delete,
        'recover'  : _render_recover,
        'login'    : _render_login,
        'logout'   : _render_dummy,
        'recover_username'  : _render_dummy,
        'reset_password'    : _render_delete,
        'update_username'   : _render_delete,
        'update_password'   : _render_delete,
        'deactivate_account': _render_delete,
        'reactivate_account': _render_delete,
    };
    var fn = extra_info_fields[item.action];
    if(fn) {
        return fn(item, uri_dict);
    } else {
        console.log('invalid action tyoe found : '+ item.action);
        return '';
    }
} // end of render_action


function _render_delete(item, uri_dict)
{
    var resource_map = {};
    resource_map[uri_dict["AuthRoleAPIView"][0]]  = {term:"roles",  uri: uri_dict["AuthRoleAPIView"],
        fn: function(x){ return x.name +"("+ x.id +")"; },};
    resource_map[uri_dict["QuotaUsageTypeAPIView"]] = {term:"quota types", uri: uri_dict["QuotaUsageTypeAPIView"],
        fn: function(x){ return x.label +"("+ x.id +")"; },};
    resource_map[uri_dict["UserGroupsAPIView"][0]]     = {term:"user groups", uri: uri_dict["UserGroupsAPIView"],
        fn: function(x){ return x.name +"("+ x.id +")"; },};
    resource_map[uri_dict["UserProfileAPIView"][0]]    = {term:"user profiles", uri: uri_dict["UserProfileAPIView"],
        fn: function(x){ return [x.first_name, x.last_name, "("+ x.id +")"].join(' '); },};
    resource_map[uri_dict["UserActivationView"]]   = {term: null,  uri: uri_dict["UserProfileAPIView"],
        fn: function(x){ return [x.first_name, x.last_name, "("+ x.id +")"].join(' '); },};
    resource_map[uri_dict["UserDeactivationView"]] = {term: null,  uri: uri_dict["UserProfileAPIView"],
        fn: function(x){ return [x.first_name, x.last_name, "("+ x.id +")"].join(' '); },};
    resource_map[uri_dict["LoginAccountCreateView"]]  = {term:"account", uri: null,
        fn: function(x){ return x.username +"("+ x.id +")"; },};
    resource_map[uri_dict["UnauthPasswordResetRequestView"]] = {term: null, uri: null, fn:null};
    resource_map[uri_dict["UnauthPasswordResetView"]]        = {term: null, uri: null,
        fn: function(x){ return x.username +"("+ x.id +")"; },};
    resource_map[uri_dict["AuthUsernameEditAPIView"]] = {term: null, uri: null,
        fn: function(x){ return x.username +"("+ x.id +")"; },};
    resource_map[uri_dict["AuthPasswdEditAPIView"]] = {term: null, uri: null,
        fn: function(x){ return x.username +"("+ x.id +")"; },};
    var out = _render_dummy(item);
    var affected_objs = [];
    var uri_key = item.uri;
    if(uri_key == uri_dict["UserActivationView"] && item.action == "update") {
        out = "request new account activation";
        affected_objs = item.affected_objs.map((x) => {
            var prof = [x.profile.first_name, x.profile.last_name, "("+ x.profile.id +")",
                ", activation page sent to email:", x.email, "<br>"]
            return  prof.join(' ');
        });
    } else if(uri_key == uri_dict["UnauthPasswordResetRequestView"] && item.action == "update") {
        out = "request reset password by generating authentication token (without login)";
        affected_objs = item.affected_objs.map((x) => {
            var prof = [x.profile.first_name, x.profile.last_name, "("+ x.profile.id +")",
                ", reset page sent to email:", x.email, "<br>"]
            return  prof.join(' ');
        });
    } else { // create, update, delete ...
        var mapitem = resource_map[uri_key];
        if(!mapitem) {
            var is_all_equal = false;
            var uri1 = uri_key.split('/');
            uri1.pop();
            var uris_with_token = [uri_dict['LoginAccountCreateView'], uri_dict['UnauthPasswordResetView']]
            for(var idx = 0; idx < uris_with_token.length; idx++) {
                var uri2 = uris_with_token[idx].split('/');
                uri2.pop();
                is_all_equal = uri1.every(function(value, jdx){ return value == uri2[jdx]; });
                if(is_all_equal) {
                    mapitem = resource_map[uris_with_token[idx]];
                    break;
                }
            }
        }
        var term = mapitem['term'];
        if(term) {
            out += ' ' + term + ', ';
        }
        affected_objs = item.affected_objs.map((x) => mapitem['fn'](x));
    }
    out += ' ' + affected_objs.join(',');
    return out;
} // end of _render_delete


function _render_recover(item, uri_dict)
{
    var out = _render_delete(item, uri_dict);
    out += item.http_status == 200 ? " succeed": " failed";
    return out;
}

function _render_login(item, uri_dict)
{
    var out = _render_dummy(item, uri_dict);
    out += item.result == "True" ? " succeed": " failed";
    return out;
}

function _render_dummy(item, uri_dict)
{
    return item.action.split('_').join(' ');
}


