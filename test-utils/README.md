Certs generated with command:
'''
openssl req -x509 -newkey rsa:4096 -sha256 -days 3650 -nodes   -keyout test.key -out test.crt -subj "/CN=localhost" -addext "subjectAltName=IP:127.0.0.1
'''
