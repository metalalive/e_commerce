from django.urls import include, path, re_path
##from django.views.generic.base import RedirectView
from ..views.common import LoginView


urlpatterns = [
    path('usermgt/',     include('web.urls.usermgt')),
    path('productmgt/',  include('web.urls.productmgt')),
    path('login',  LoginView.as_view()  ),
    #re_path(r'.', RedirectView.as_view(url='/')),
]


