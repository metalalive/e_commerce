from django.conf import settings as django_settings
from django.core.exceptions import PermissionDenied
from django.views.generic.base import View, ContextMixin, TemplateResponseMixin
from django.middleware.csrf import rotate_token
from django.contrib.auth.views import redirect_to_login
from django.contrib.auth.mixins import LoginRequiredMixin

NON_FIELD_ERRORS_KEY = "non_field_errors"


class ExtendedTemplateResponseMixin(TemplateResponseMixin):
    """
    extend functions from TemplateResponseMixin, to support
    multi-template selection in a view class.
    """

    def render_to_response(self, context, **response_kwargs):
        """
        add optional extra keyword argument : chosen_template_idx,
        in general usage scenario, only one template will be chosen for each request
        on each view class.
        """
        response_kwargs.setdefault("content_type", self.content_type)
        template_names = self.get_template_names()
        template_names_len = len(template_names)
        chosen_template_idx = response_kwargs.pop("chosen_template_idx", 0)
        if chosen_template_idx >= template_names_len:
            raise IndexError(
                "chosen template index : %s, total length of template name list: %s"
                % (chosen_template_idx, template_names_len)
            )
        return self.response_class(
            request=self.request,
            template=template_names[chosen_template_idx],
            context=context,
            using=self.template_engine,
            **response_kwargs
        )

    def get_template_names(self):
        if not self.template_name:  # for None, empty list/tuple, empty string
            raise ImproperlyConfigured(
                "TemplateResponseMixin requires either a definition of "
                "'template_name' or an implementation of 'get_template_names()'"
            )
        elif isinstance(self.template_name, (list, tuple)):
            return self.template_name
        else:  # default valid value is non-empty string
            return [self.template_name]


class ExtendedLoginRequiredMixin(LoginRequiredMixin):
    def handle_no_permission(self):
        """
        extended from original Django implementation, to support cross-domain redirect URL
        """
        if self.raise_exception or self.request.user.is_authenticated:
            raise PermissionDenied(self.get_permission_denied_message())
        # TODO, auth_host might not come from django app, it might be flask or other frameworks
        auth_hosts = getattr(django_settings, "AUTH_HOSTS", [])
        if auth_hosts and auth_hosts[0]:
            toward_url = "%s://%s%s" % (
                self.request.scheme,
                self.request._get_raw_host(),
                self.request.get_full_path(),
            )
            login_url = "%s://%s:%s%s" % (
                self.request.scheme,
                auth_hosts[0][0],
                auth_hosts[0][1],
                self.get_login_url(),
            )
            print("(toward_url, login_url) = (%s, %s)" % (toward_url, login_url))
        else:
            toward_url = self.request.get_full_path()
            login_url = self.get_login_url()
        return redirect_to_login(toward_url, login_url, self.get_redirect_field_name())


# from django.contrib.auth.decorators import login_required, permission_required
# django view decorator cannot be used in REST framework view
# @permission_required(perm=('auth.add_group',), login_url=CUSTOM_LOGIN_URL,)
# @login_required(login_url=CUSTOM_LOGIN_URL, redirect_field_name='redirect')


class BaseHTMLView(View, ContextMixin, ExtendedTemplateResponseMixin):
    """
    base html view for any user, all subclasses may provide application-specific
    context data to render the template
    """

    extra_context = None

    def get(self, request, *args, **kwargs):
        formparams = kwargs.pop("formparams", {})
        response_kwargs = kwargs.pop("response_kwargs", {})
        context = self.get_context_data(formparams=formparams)
        return self.render_to_response(context=context, **response_kwargs)


class BaseAuthHTMLView(ExtendedLoginRequiredMixin, BaseHTMLView):
    """base html view for authorized users"""

    redirect_field_name = "redirect"
    login_url = None  # subclasses must override this attribute


class BaseLoginHTMLView(BaseHTMLView):
    template_name = None
    submit_url = None
    default_url_redirect = None

    def __new__(cls, *args, **kwargs):
        assert (
            cls.template_name and cls.submit_url and cls.default_url_redirect
        ), "class variables not properly configured"
        return super().__new__(cls, *args, **kwargs)

    def get(self, request, *args, **kwargs):
        if "CSRF_COOKIE" not in request.META:
            rotate_token(request=request)
            request.csrf_cookie_age = 7200
        redirect = request.GET.get(
            BaseAuthHTMLView.redirect_field_name, self.default_url_redirect
        )
        kwargs["formparams"] = {
            "non_field_errors": NON_FIELD_ERRORS_KEY,
            "submit_url": self.submit_url,
            "redirect": redirect,
        }
        return super().get(request, *args, **kwargs)
