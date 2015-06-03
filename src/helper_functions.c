#include "helper_functions.h"


xmlNodePtr xmlNextSibling(const xmlNodePtr cur) {
    return cur->next;
}

xmlNodePtr xmlPrevSibling(const xmlNodePtr cur) {
    return cur->prev;
}

xmlNodePtr xmlGetFirstChild(const xmlNodePtr cur) {
    return cur->children;
}

const char * xmlNodeGetName(const xmlNodePtr cur) {
    return (char *) cur->name;
}

const char * xmlNodeGetContentPointer(const xmlNodePtr cur) {
    return (char *) cur->content;
}
