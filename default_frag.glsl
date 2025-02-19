precision mediump float; // might be 16-bit? don't set globally?
in vec2 uv;
out vec4 color;
uniform float t;

float mult(float x, float y) {
    return x*y;
}

float add(float x, float y) {
    return x+y;
}

// sigmoid
float sig(float x, float x0, float r) {
    // rescale -1,1 input range to 5,15
    float rs = r+1.0;
    rs *= 7.5;
    rs +=5.0;
    return 1.0/(1.0 + exp(-(rs*(x-x0))));
}

void main() {
    float u = uv.x;
    float v = uv.y;
    float r = sqrt(u*u + v*v);
    float red = mult(add(sin(t),1.0),0.5);
    float green = v;
    float blue = u;
    color = vec4(red, green, blue, 1.0);
}
