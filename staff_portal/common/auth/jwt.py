from datetime import datetime, timedelta, timezone
import math
import logging
import jwt

from common.util.python  import ExtendedDict

_logger = logging.getLogger(__name__)


class JWT:
    """
    internal wrapper class for detecting JWT write, verify, and generate encoded token,
    in this wrapper, `acc_id` claim is required in payload
    """

    def __init__(self, encoded=None):
        self.encoded = encoded
        self._destroy = False
        self._valid = None

    @property
    def encoded(self):
        return self._encoded

    @encoded.setter
    def encoded(self, value):
        self._encoded = value
        if value:
            header = jwt.get_unverified_header(value)
            payld  = jwt.decode(value, options={'verify_signature':False})
        else:
            header = {}
            payld  = {}
        self._payld  = ExtendedDict(payld)
        self._header = ExtendedDict(header)

    @property
    def payload(self):
        return self._payld

    @property
    def header(self):
        return self._header

    @property
    def modified(self):
        return self.header.modified or self.payload.modified

    @property
    def valid(self):
        """ could be True, False, or None (not verified yet) """
        return self._valid

    @property
    def destroy(self):
        return self._destroy

    @destroy.setter
    def destroy(self, value:bool):
        self._destroy = value

    def verify(self, keystore, audience, unverified=None):
        self._valid = False
        if unverified:
            self.encoded = unverified
        alg = self.header.get('alg', '')
        unverified_kid = self.header.get('kid', '')
        keyitem = keystore.choose_pubkey(kid=unverified_kid)
        pubkey = keyitem['key']
        if not pubkey:
            log_args = ['unverified_kid', unverified_kid, 'alg', alg,
                    'msg', 'public key not found on verification',]
            _logger.warning(None, *log_args) # log this because it may be security issue
            return
        try:
            options = {'verify_signature': True, 'verify_exp': True, 'verify_aud': True,}
            verified = jwt.decode(self.encoded, pubkey, algorithms=alg,
                        options=options, audience=audience)
            errmsg = 'data inconsistency, self.payload = %s , verified = %s'
            assert self.payload == verified, errmsg % (self.payload, verified)
            self._valid = True
        except Exception as e:
            log_args = ['encoded', self.encoded, 'pubkey', pubkey, 'err_msg', e]
            _logger.warning(None, *log_args)
            verified = None
        return verified


    def encode(self, keystore):
        if self.modified:
            log_args = []
            unverified_kid = self.header.get('kid', '')
            keyitem  = keystore.choose_secret(kid=unverified_kid, randomly=True)
            if keyitem.get('kid', None) and unverified_kid != keyitem['kid']:
                log_args.extend(['unverified_kid', unverified_kid, 'verified_kid', keyitem['kid']])
            self.header['kid'] = keyitem.get('kid', unverified_kid)
            # In PyJwt , alg can be `RS256` (for RSA key) or `HS256` (for HMAC key)
            self.header['alg'] = keyitem['alg']
            secret = keyitem['key']
            if secret:
                out = jwt.encode(self.payload, secret, algorithm=self.header['alg'],
                        headers=self.header)
            log_args.extend(['alg', keyitem['alg'], 'encode_succeed', any(out), 'secret_found', any(secret)])
            _logger.debug(None, *log_args)
        else:
            out = self.encoded
        return out

    def default_claims(self, header_kwargs, payld_kwargs):
        self.header.update(header_kwargs, overwrite=False)
        self.payload.update(payld_kwargs, overwrite=False)


